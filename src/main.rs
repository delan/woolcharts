use std::collections::BTreeMap;
use std::fmt::Debug;
use std::{io::Read, fs::File, env::args};
use std::str::FromStr;

use regex::Regex;
use scraper::{Html, Selector, ElementRef};

lazy_static::lazy_static! {
    static ref DATE: Selector = Selector::parse("meta[name=date]").unwrap();
    static ref P: Selector = Selector::parse("p").unwrap();
    static ref PAGE: Selector = Selector::parse("div[id^=page][id$=-div]").unwrap();

    static ref TOP: Regex = Regex::new("top:([^p]+)px").unwrap();
    static ref LEFT: Regex = Regex::new("left:([^p]+)px").unwrap();
    static ref ITEM_NAME: Regex = Regex::new(" *(?:(?:[*]|[~]|[(]Sub[)]) )*(.+)").unwrap();

    static ref MEAT: Regex = Regex::new("(?P<name>.+) (?:[^ ]+k?g) - (?P<factor>[^ ]+k?g)").unwrap();
    static ref UNITLESS_PRICE: Regex = Regex::new("[$]([0-9]+[.][0-9][0-9])").unwrap();
    static ref PRICE_PER_KG: Regex = Regex::new("[$]([0-9]+[.][0-9][0-9])/Kg").unwrap();
}

struct Price(usize, usize);
impl Price {
    fn cents(cents: f64) -> Self {
        Self((cents / 100.0) as _, (cents % 100.0) as _)
    }
    fn normalised(name: &str, price: &str) -> Self {
        let (cents, per_kg) = if let Some(x) = PRICE_PER_KG.captures(price) {
            (x.get(1).unwrap().as_str().replace(".", ""), true)
        } else if let Some(x) = UNITLESS_PRICE.captures(price) {
            (x.get(1).unwrap().as_str().replace(".", ""), false)
        } else {
            panic!("bad price: {}", price)
        };

        let factor = if !per_kg {
            1.0
        } else if let Some(meat) = MEAT.captures(name) {
            let factor = meat.name("factor").unwrap().as_str();
            let factor = if let Some(x) = factor.strip_suffix("kg") {
                x.parse::<f64>().unwrap()
            } else if let Some(x) = factor.strip_suffix("g") {
                x.parse::<f64>().unwrap() / 1000.0
            } else {
                unreachable!()
            };
            factor
        } else {
            panic!("price is $/Kg but name is meat")
        };

        Self::cents(cents.parse::<f64>().unwrap() * factor)
    }
}
impl Debug for Price {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "${}.{:02}", self.0, self.1)
    }
}

fn main() -> eyre::Result<()> {
    let mut items = BTreeMap::default();

    for path in args().skip(1) {
        for (date, name, price) in dump(&dbg!(path))? {
            items.entry(name)
                .or_insert(BTreeMap::default())
                .insert(date, price);
        }
    }

    dbg!(items);

    Ok(())
}

fn dump(path: &str) -> eyre::Result<Vec<(String, String, Price)>> {
    let mut input = String::default();
    File::open(path)?.read_to_string(&mut input)?;
    let input = Html::parse_fragment(&input);
    let date = input.select(&DATE).next().unwrap().value().attr("content").unwrap();
    dbg!(date);

    // Collect all of the text, ordered by CSS (top,left).
    let mut ps = vec![];
    for page in input.select(&PAGE) {
        let mut page = page.select(&P).collect::<Vec<_>>();
        page.sort_by(|p, q| {
            let (px, py) = coords(p);
            let (qx, qy) = coords(q);
            py.total_cmp(&qy).then(px.total_cmp(&qx))
        });
        for p in page {
            ps.push(p);
        }
    }

    let mut state = 0;
    let mut first_item_y = None;
    let mut first_item_xs = vec![];
    let mut items = vec![];
    let mut item = vec![];
    let mut prev_column_index = 0;
    for p in &ps {
        state = match (state, p.text().collect::<Vec<_>>().join("").as_ref()) {
            (0, "Supplied") => 1,
            (1, "Line") => 2,
            (2, "Description") => 3,
            (3, "Ordered") => 4,
            (4, "Supplied") => 5,
            (5, "Price") => 6,
            (6, "Amount") => 7,
            (7, "Sub\u{A0}Total:") => 0,
            (7, x) if x.starts_with("Registered\u{A0}Office:\u{A0}") => 0,
            (0, _) => 0,
            (7, _) => 7,
            (_, _) => 0,
        };
        if state == 0 {
            first_item_y = None;
            first_item_xs = vec![];
        }
        if state < 7 {
            continue;
        }

        // Get x coordinates for the columns based on those of the first item.
        if first_item_y.is_none() && p.first_child().map_or(false, |x| x.value().is_text()) {
            let (_, py) = coords(p);
            first_item_y = Some(py);
        }
        if let Some(y) = first_item_y {
            let (px, py) = coords(p);
            if py == y {
                first_item_xs.push(px);
            }
        }

        // Process cell value.
        if let Some(text) = p.first_child().and_then(|x| x.value().as_text()) {
            let (px, _) = coords(p);
            let i = first_item_xs.iter()
                .map(|x| (x - px).abs())
                .enumerate()
                .min_by(|(_, p), (_, q)| p.total_cmp(q))
                .unwrap().0;
            // eprintln!("{} {:?} {:?}", i, text, p.value());
            if i <= prev_column_index {
                add_item(item, &mut items);
                item = vec![];
            }
            while item.len() < i {
                item.push(None);
            }
            let text = text.replace("\u{A0}", " ");
            let mut text = &*text;
            if i == 1 {
                text = ITEM_NAME.captures(text).unwrap().get(1).unwrap().as_str();
            }
            item.push(Some(text.to_owned()));
            prev_column_index = i;
        }
    }
    add_item(item, &mut items);

    let mut result = vec![];
    for item in &items {
        eprintln!("({})", item.iter().map(|x| match x {
            Some(x) => format!("{:?}", x),
            None => "_".to_owned(),
        }).collect::<Vec<_>>().join(", "));
        if let (Some(name), Some(price)) = (&item[1], &item[4]) {
            result.push((date.to_owned(), name.clone(), Price::normalised(name, price)));
        }
    }

    Ok(result)
}

fn coords(element: &ElementRef) -> (f64, f64) {
    let style = element.value().attr("style").unwrap();
    let x = LEFT.captures(style).unwrap().get(1).unwrap().as_str();
    let y = TOP.captures(style).unwrap().get(1).unwrap().as_str();
    let x = f64::from_str(x).unwrap();
    let y = f64::from_str(y).unwrap();

    (x, y)
}

fn add_item(mut item: Vec<Option<String>>, items: &mut Vec<Vec<Option<String>>>) {
    const COLUMNS: usize = 6;
    if item.is_empty() {
        return;
    }
    while item.len() < COLUMNS {
        item.push(None);
    }
    items.push(item);
}

use std::{io::{stdin, Read}, error::Error};
use std::str::FromStr;

use regex::Regex;
use scraper::{Html, Selector, ElementRef};

lazy_static::lazy_static! {
    static ref DATE: Selector = Selector::parse("meta[name=date]").unwrap();
    static ref P: Selector = Selector::parse("p").unwrap();
    static ref PAGE: Selector = Selector::parse("div[id^=page][id$=-div]").unwrap();

    static ref TOP: Regex = Regex::new("top:([^p]+)px").unwrap();
    static ref LEFT: Regex = Regex::new("left:([^p]+)px").unwrap();
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut input = String::default();
    stdin().lock().read_to_string(&mut input)?;
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
            if i <= prev_column_index && !item.is_empty() {
                items.push(item);
                item = vec![];
            }
            while item.len() < i {
                item.push(None);
            }
            let text = match (i, text.replace("\u{A0}", " ")) {
                (1, x) if x.starts_with("* ") => x[2..].to_owned(),
                (1, x) if x.starts_with("   ~ ") => x[5..].to_owned(),
                (_, x) => x,
            };
            item.push(Some(text));
            prev_column_index = i;
        }
    }

    for item in &items {
        eprintln!("({})", item.iter().map(|x| match x {
            Some(x) => format!("{:?}", x),
            None => "_".to_owned(),
        }).collect::<Vec<_>>().join(", "));
    }

    Ok(())
}

fn coords(element: &ElementRef) -> (f64, f64) {
    let style = element.value().attr("style").unwrap();
    let x = LEFT.captures(style).unwrap().get(1).unwrap().as_str();
    let y = TOP.captures(style).unwrap().get(1).unwrap().as_str();
    let x = f64::from_str(x).unwrap();
    let y = f64::from_str(y).unwrap();

    (x, y)
}

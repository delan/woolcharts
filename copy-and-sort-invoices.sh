#!/bin/sh
set -eu
mkdir -p sorted
for path; do
    filename=${path##*/}
    basename=${filename%%.*}
    extension=${filename#*.}
    case "$basename" in
        # WOW_invoice_<customer>_<order>, like in personal data requests (SID-<customer>.zip)
        (WOW_invoice_*_*)
            order=$(printf \%s "$basename" | cut -d _ -f 4)
            ;;
        # <order>-<128-bit hash>, like in website downloads until those were removed in ~2023
        (*-????????????????????????????????)
            order=$(printf \%s "$basename" | cut -d - -f 1)
            ;;
    esac
    # Zero-pad order numbers to nine digits.
    order=$(printf \%09d "$order")
    set -- cp -- "$path" "sorted/$order.$extension"
    echo "$@"
    "$@"
done

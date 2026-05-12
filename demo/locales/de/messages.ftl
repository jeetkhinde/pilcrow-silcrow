# ── Navigation ────────────────────────────────────────────────
nav-home = Startseite
nav-products = Produkte
nav-about = Über uns

# ── Common ────────────────────────────────────────────────────
welcome = Willkommen bei Pilcrow
greeting = Hallo, { $name }!

# ── Products ──────────────────────────────────────────────────
product-count = { $count ->
    [one] { $count } Produkt
   *[other] { $count } Produkte
}

product-price = { $amount } { $currency }

# ── Errors ────────────────────────────────────────────────────
error-required = { $field } ist erforderlich.
error-invalid  = { $field } ist ungültig.

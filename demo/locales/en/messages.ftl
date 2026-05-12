# ── Navigation ────────────────────────────────────────────────
nav-home = Home
nav-products = Products
nav-about = About

# ── Common ────────────────────────────────────────────────────
welcome = Welcome to Pilcrow
greeting = Hello, { $name }!

# ── Products ──────────────────────────────────────────────────
product-count = { $count ->
    [one] { $count } product
   *[other] { $count } products
}

product-price = { $currency }{ $amount }

# ── Errors ────────────────────────────────────────────────────
error-required = { $field } is required.
error-invalid  = { $field } is invalid.

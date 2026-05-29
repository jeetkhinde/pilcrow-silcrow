# Directives and Bindings

Silcrow behavior is declared in HTML attributes.

## Bindings

Common bindings:

- `:text`
- `:class`
- `:style`
- `:show`
- `:value`
- `:checked`
- `:disabled`
- `:selected`
- `:hidden`
- `:required`
- `:readOnly`
- `:src`
- `:href`
- `:selectedIndex`
- `:key`

## Spread

Use `s-use` to spread object values into bindings.

## Loops

Use `s-for` for keyed list rendering:

```html
<template s-for="item in items" :key="item.id">
  <li :text="item.title"></li>
</template>
```

## Debug

Use `s-debug` when inspecting binding state.

## Safety

Silcrow validates URL protocols, hardens `_blank` links, blocks prototype pollution paths, and sanitizes unsafe HTML surfaces. Do not bypass these defaults in examples.

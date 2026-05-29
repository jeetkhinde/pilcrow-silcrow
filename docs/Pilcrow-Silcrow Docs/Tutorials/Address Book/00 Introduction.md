# Address Book Tutorial

Build a two-column contact manager in Pilcrow + Silcrow — the same app as the [React Router address book tutorial](https://reactrouter.com/tutorials/address-book), rebuilt with Rust server-side rendering, file routes, named actions, and FSR live fields.

## What you will build

![[screenshot-placeholder]]

A full contact manager with:

- Sidebar with contact list, search, and a New button
- Contact detail pane with avatar, Twitter link, and notes
- Favourite star that toggles without a page reload
- Edit form with cancel
- Delete with confirmation
- Live-updating contact count and timestamps via FSR + SSE
- Client-side navigation — only the right pane swaps, sidebar stays
- A loading skeleton that fills the detail pane while the next contact loads

## Steps

1. [[01 Setup]]
2. [[02 Root Layout]]
3. [[03 Route Groups and the App Layout]]
4. [[04 Loading Data]]
5. [[05 Contact Detail]]
6. [[06 Creating Contacts]]
7. [[07 Client-Side Navigation]]
8. [[08 Search]]
9. [[09 Active Link Styling]]
10. [[10 Edit Form]]
11. [[11 Favourite Toggle]]
12. [[12 Deleting Contacts]]
13. [[13 FSR Live Fields]]
14. [[14 Live Timestamps on the Edit Page]]
15. [[15 Loading Skeletons]]

## Prerequisites

- Rust + Cargo installed
- Postgres running locally
- Redis running locally

## Source code

The finished app lives in `address-book/` in the workspace root.

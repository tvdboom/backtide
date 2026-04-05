# Currencies
------------


## Currency conversion

- "direct": Always use a direct conversion if it exists, e.g., `JPY`->`EUR`.
- "earliest": Take the conversion with the longest history, even if that
  means taking an extra step through [`triangulation_fiat`][generalconfig],
  e.g., `JPY`->`USD` and `USD`->`EUR`.

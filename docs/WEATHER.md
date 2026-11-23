# Destination Weather (development feature)

Atlas can preview forecast conditions for the destination of an upcoming saved stay in **Map → Weather ahead**. This branch adds a weather forecast panel without modifying the current signed 0.6.0 release candidate or its trusted signing identity.

## Use

1. Open **Map → Weather ahead**.
2. Select an upcoming stay that has coordinates, or manually enter a **city or region** to resolve via Open-Meteo geocoding. The saved street address is *not* sent for geocoding.
3. Choose Celsius / km/h or Fahrenheit / mph.
4. Read up to **16 forecast days**. Each day displays its condition, high/low temperature, average forecast humidity derived from hourly readings, maximum wind speed and maximum forecast rain probability.
5. Click a day to open its **hourly forecast**, showing local hour, sky/precipitation conditions, temperature, humidity, wind speed and direction, and probability of precipitation.
6. Use **Refresh** to fetch newer data. An in-memory 30-minute cache avoids redundant requests.

Forecasts are estimates and require Internet access. The feature uses **Open-Meteo**'s public forecast and geocoding APIs, with provider attribution in the UI. Provider availability, terms, and noncommercial eligibility should be reviewed before any commercial distribution.

## Forecast horizon and trip dates

Open-Meteo supports forecasts extending up to approximately 16 days. When the selected stay's arrival is beyond the returned range, Atlas explicitly says the near-term forecast does **not** predict the stay date. Weather data is not stored as a booking fact or used to change reservations automatically.

## Privacy and security

- Weather uses fixed HTTPS-only Open-Meteo endpoints and denies redirects.
- Only city queries entered by the owner or saved coordinates rounded to two decimal places are sent to the provider; **no street address, mailbox content, record ID, device GPS, or booking information** is uploaded.
- The desktop and localhost development host can request forecasts, while the authenticated phone companion does **not** receive an unrestricted weather/network RPC.
- Responses have timeouts and a 512-KiB upper bound. Requests run outside the SQLite mutex.
- The app's normal records, calendars and budgets remain functional while offline; forecast cards correctly report connection errors.

## Test and delivery status

Source changes are on `codex/atlas-v0.7-destination-weather`, stacked on the reviewed V0.6 development branch. This is **not** a published signed release and is not yet installed on the owner's machine. Tests should cover valid and invalid city/coordinate requests, no-address-leak behavior, 16-day data shape, date selection, provider failures, cache behavior, time-zone labels, and narrow viewport layouts. Complete the normal Rust/TypeScript/browser and native CI checks before merging. The signed release workflow still requires owner review and publication; do not substitute expiring Actions ZIPs for a stable production update feed.

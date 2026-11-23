import { useEffect, useMemo, useState, type FormEvent } from "react";
import { CloudSun, RefreshCw, Search, CloudRain, Sun, Cloud, CloudSnow, Wind, Droplets } from "lucide-react";
import { AtlasRecord, rpc } from "./api";
import "./weather.css";

type Daily = Record<string, (string | number | null)[]>;
type Hourly = Record<string, (string | number | null)[]>;
type Forecast = { source: string; sourceUrl: string; fetchedAt: string; timezone: string; daily: Daily; hourly: Hourly; dailyUnits?: Record<string, string>; hourlyUnits?: Record<string, string> };
type City = { name: string; region?: string; country?: string; latitude: number; longitude: number; timezone?: string };
type Destination = { label: string; latitude: number; longitude: number; arrival?: string };
const forecastCache = new Map<string, { data: Forecast; expires: number }>();
const cacheMs = 30 * 60 * 1000;

function condition(code: unknown) {
  if (typeof code !== "number") return { label: "Unknown", Icon: Cloud };
  if (code === 0) return { label: "Clear sky", Icon: Sun };
  if (code >= 1 && code <= 3) return { label: code === 3 ? "Overcast" : "Partly cloudy", Icon: CloudSun };
  if (code === 45 || code === 48) return { label: "Fog", Icon: Cloud };
  if ([51, 53, 55, 56, 57].includes(code)) return { label: "Drizzle", Icon: CloudRain };
  if ([61, 63, 65, 66, 67, 80, 81, 82].includes(code)) return { label: "Rain", Icon: CloudRain };
  if ([71, 73, 75, 77, 85, 86].includes(code)) return { label: "Snow", Icon: CloudSnow };
  if ([95, 96, 99].includes(code)) return { label: "Thunderstorm", Icon: CloudRain };
  return { label: "Variable conditions", Icon: CloudSun };
}
const n = (v: unknown, decimals = 0): string => typeof v === "number" && Number.isFinite(v) ? v.toFixed(decimals) : "—";
function dailyMean(values: (string | number | null)[] | undefined, timestamps: (string | number | null)[] | undefined, date: string) {
  if (!values || !timestamps) return null;
  const valid = timestamps.reduce<number[]>((out, time, index) => {
    if (typeof time === "string" && time.slice(0,10) === date && typeof values[index] === "number")
      out.push(values[index] as number);
    return out;
  }, []);
  return valid.length ? valid.reduce((sum, x) => sum + x, 0) / valid.length : null;
}
function displayDate(day: string) {
  const value = new Date(day + "T12:00:00Z");
  return Number.isFinite(value.valueOf()) ? new Intl.DateTimeFormat("en-US", {weekday:"short",month:"short",day:"numeric",timeZone:"UTC"}).format(value) : day;
}
function savedDestinations(records: AtlasRecord[]): { id: string; label: string; latitude?: number; longitude?: number; arrival?: string }[] {
  const now = new Date().toISOString().slice(0,10);
  return records.filter((r) => r.kind === "stay" && r.status !== "cancelled" && r.checkOut >= now)
    .sort((a,b) => (a.checkIn || "").localeCompare(b.checkIn || ""))
    .map((r) => ({ id:r.id, label:r.name || r.address || "Saved stay", latitude:r.latitude, longitude:r.longitude, arrival:r.checkIn }));
}
export function WeatherPanel({ records }: { records: AtlasRecord[] }) {
  const stays = useMemo(() => savedDestinations(records), [records]);
  const [selectedId, setSelectedId] = useState("");
  const [manual, setManual] = useState<Destination | null>(null);
  const [cityInput, setCityInput] = useState("");
  const [cities, setCities] = useState<City[]>([]);
  const [searching, setSearching] = useState(false);
  const [temperatureUnit, setTemperatureUnit] = useState<"celsius" | "fahrenheit">("celsius");
  const [forecast, setForecast] = useState<Forecast | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [selectedDate, setSelectedDate] = useState("");
  const [reload, setReload] = useState(0);
  const stay = stays.find((s) => s.id === selectedId);
  const available = !!stay && typeof stay.latitude === "number" && Number.isFinite(stay.latitude) && typeof stay.longitude === "number" && Number.isFinite(stay.longitude);
  const destination: Destination | null = available
    ? { label: stay!.label, latitude:stay!.latitude!, longitude:stay!.longitude!, arrival:stay!.arrival }
    : manual;
  // Selecting a saved stay without coordinates never uploads its address.
  const selectedDisplay = stay?.label || destination?.label || "Select a destination";
  const key = destination ? `${destination.latitude.toFixed(2)},${destination.longitude.toFixed(2)}:${temperatureUnit}` : "";

  useEffect(() => {
    if (!selectedId && stays.length) setSelectedId(stays[0].id);
  }, [stays,selectedId]);
  useEffect(() => {
    if (!destination) { setForecast(null); setError(""); return; }
    let active = true;
    const hit = forecastCache.get(key);
    if (hit && hit.expires > Date.now() && reload === 0) {
      setForecast(hit.data);
      setLoading(false);
      setError("");
      return;
    }
    setLoading(true); setError(""); setForecast(null);
    void rpc<Forecast>("weather.forecast", { latitude: destination.latitude, longitude: destination.longitude, temperatureUnit })
      .then((data) => {
        if (!active) return;
        setForecast(data);
        forecastCache.set(key,{data,expires:Date.now()+cacheMs});
        const days = data.daily.time || [];
        const arrival = destination.arrival || "";
        const chosen = days.includes(arrival) ? arrival : days[0];
        setSelectedDate(typeof chosen === "string" ? chosen : "");
      })
      .catch((reason) => { if (active) setError(String(reason instanceof Error ? reason.message : reason)); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [key,reload]);

  async function findCity(event: FormEvent) {
    event.preventDefault();
    setSearching(true); setCities([]); setError("");
    try {
      const data = await rpc<{cities:City[]}>("weather.search", {city:cityInput});
      setCities(data.cities);
      if (!data.cities.length) setError("No matching destination found. Try city and country.");
    } catch (reason) { setError(String(reason instanceof Error ? reason.message : reason)); }
    finally { setSearching(false); }
  }
  const dates = (forecast?.daily.time || []) as string[];
  const dateIndex = Math.max(0,dates.indexOf(selectedDate));
  const day = dates[dateIndex];
  const timeValues = (forecast?.hourly.time || []) as string[];
  const rows = timeValues.flatMap((time,i) => typeof time === "string" && time.slice(0,10) === day ? [{time,index:i}] : []);
  const arrival = destination?.arrival;
  const arrivalOutOfRange = !!arrival && !!dates.length && !dates.includes(arrival);
  const temp = temperatureUnit === "celsius" ? "°C" : "°F";
  const windUnit = temperatureUnit === "celsius" ? "km/h" : "mph";

  return (
    <section className="atlas-weather" aria-label="Destination weather forecast">
      <div className="weather-heading">
        <div>
          <span className="eyebrow">DESTINATION CONDITIONS</span>
          <h2><CloudSun size={23}/> Weather ahead</h2>
          <p className="muted">Choose an upcoming stay or select a city. No device location or precise street address is sent.</p>
        </div>
        <div className="weather-controls">
          <label>Units
            <select aria-label="Temperature units" value={temperatureUnit} onChange={e=>setTemperatureUnit(e.target.value as "celsius" | "fahrenheit")}>
              <option value="celsius">°C · km/h</option><option value="fahrenheit">°F · mph</option>
            </select>
          </label>
          <button className="secondary" disabled={!destination || loading} onClick={()=>setReload(n=>n+1)}><RefreshCw size={14}/> Refresh</button>
        </div>
      </div>
      <label className="weather-stay-label">Upcoming accommodation
        <select value={selectedId} onChange={e=>{setSelectedId(e.target.value);setManual(null);setCities([]);setReload(0);}}>
          <option value="manual">Select a city manually</option>
          {stays.map(s=><option key={s.id} value={s.id}>{s.label}{s.arrival ? ` · ${s.arrival}` : ""}</option>)}
        </select>
      </label>
      {!available && <>
        <p className="muted">{stay ? "This stay has no saved coordinates. Select its city manually; Atlas will not send the street address." : "Search for your next destination."}</p>
        <form onSubmit={findCity} className="weather-search">
          <input aria-label="Destination city" value={cityInput} onChange={e=>setCityInput(e.target.value)} placeholder="City or region, e.g. Lisbon" maxLength={80} required/>
          <button className="secondary" type="submit" disabled={searching || cityInput.trim().length < 2}><Search size={15}/>{searching ? "Searching…" : "Find city"}</button>
        </form>
        {!!cities.length && <div className="weather-cities" aria-label="Matching destinations">{cities.map((city,i)=>
          <button key={i} className="secondary" onClick={()=>{
            setManual({label:[city.name,city.region,city.country].filter(Boolean).join(", "),latitude:city.latitude,longitude:city.longitude,arrival:stay?.arrival});
            setCities([]);setReload(0);
          }}>{[city.name,city.region,city.country].filter(Boolean).join(", ")}</button>
        )}</div>}
      </>}
      {error && <p role="alert" className="error-banner">{error}</p>}
      {loading && <p role="status" className="muted">Retrieving the latest weather forecast…</p>}
      {forecast && !!dates.length && <>
        <p className="weather-source"><strong>{selectedDisplay}</strong> · Times in {forecast.timezone} · Updated {new Date(forecast.fetchedAt).toLocaleString()} · Forecasts are estimates, not observations</p>
        {arrivalOutOfRange && <p className="weather-note">Your planned arrival ({arrival}) is outside the currently available forecast window ({dates[0]} to {dates[dates.length-1]}). The days below show the near-term forecast for this destination, not the weather on your arrival date.</p>}
        <div className="weather-days" role="group" aria-label="Daily destination weather">
          {dates.map((date,index)=>{
            const code=forecast.daily.weather_code?.[index];
            const {label,Icon}=condition(code);
            const humidity=dailyMean(forecast.hourly.relative_humidity_2m,timeValues,date);
            return <button key={date} className={`weather-day ${day===date?"selected":""}`} aria-pressed={day===date} onClick={()=>setSelectedDate(date)}>
              <strong>{displayDate(date)}</strong>
              <Icon size={22} strokeWidth={1.6}/>
              <span>{label}</span>
              <b>{n(forecast.daily.temperature_2m_max?.[index])}° / {n(forecast.daily.temperature_2m_min?.[index])}°</b>
              <small><Droplets size={12}/> {humidity===null?"—":n(humidity)+"%"} humidity</small>
              <small><Wind size={12}/> {n(forecast.daily.wind_speed_10m_max?.[index])} {windUnit}</small>
              <small><CloudRain size={12}/> {n(forecast.daily.precipitation_probability_max?.[index])}% rain</small>
            </button>;
          })}
        </div>
        {day && <div className="weather-hourly">
          <h3>Hourly forecast · {displayDate(day)}</h3>
          <p className="muted">Estimated conditions at each hour in the destination's local time.</p>
          <div className="weather-hourly-scroll">
            <table><thead><tr><th>Time</th><th>Conditions</th><th>Temp.</th><th>Humidity</th><th>Wind</th><th>Direction</th><th>Rain chance</th></tr></thead><tbody>
              {rows.map(({time,index})=>{
                const {label,Icon}=condition(forecast.hourly.weather_code?.[index]);
                return <tr key={time}><td>{time.slice(11,16)}</td><td><span className="weather-condition"><Icon size={15}/>{label}</span></td><td>{n(forecast.hourly.temperature_2m?.[index])}{temp}</td><td>{n(forecast.hourly.relative_humidity_2m?.[index])}%</td><td>{n(forecast.hourly.wind_speed_10m?.[index])} {windUnit}</td><td>{n(forecast.hourly.wind_direction_10m?.[index])}°</td><td>{n(forecast.hourly.precipitation_probability?.[index])}%</td></tr>;
              })}
            </tbody></table>
          </div>
        </div>}
        <p className="weather-attribution">Data provided by <a href={forecast.sourceUrl} target="_blank" rel="noopener noreferrer">Open-Meteo</a>. Forecasts are available for approximately 16 days; longer-range trips require checking again closer to departure. Weather requires internet access.</p>
      </>}
    </section>
  );
}

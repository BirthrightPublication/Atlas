import { useState } from "react";
import {
  ArrowUpRight,
  Search,
  Bookmark,
  Plus,
  Compass,
  Check,
  HelpCircle,
  MapPin,
  ArrowLeftRight,
  X,
} from "lucide-react";
import {
  OfferComparison,
  PreparedSearch,
  WatchlistActions,
} from "./OfferTools";
import { AtlasRecord, Settings, money, nights, openExternal, rpc } from "./api";

const PROVIDERS = [
  ["Airbnb", "Stays", "https://www.airbnb.com/"],
  ["Booking.com", "Stays", "https://www.booking.com/"],
  ["Agoda", "Stays", "https://www.agoda.com/"],
  ["Hostelworld", "Stays", "https://www.hostelworld.com/"],
  ["Flatio", "Stays", "https://www.flatio.com/"],
  ["Skyscanner", "Travel", "https://www.skyscanner.net/"],
  ["Rome2Rio", "Travel", "https://www.rome2rio.com/"],
  ["Omio", "Travel", "https://www.omio.com/"],
  ["Trainline", "Travel", "https://www.thetrainline.com/"],
  ["12Go", "Travel", "https://12go.asia/"],
  ["Ferryhopper", "Travel", "https://www.ferryhopper.com/"],
  ["GetYourGuide", "Experiences", "https://www.getyourguide.com/"],
  ["Viator", "Experiences", "https://www.viator.com/"],
  ["Klook", "Experiences", "https://www.klook.com/"],
  ["Atlas Obscura", "Experiences", "https://www.atlasobscura.com/"],
  ["Eventbrite", "Experiences", "https://www.eventbrite.com/"],
  ["Meetup", "Experiences", "https://www.meetup.com/"],
  ["iOverlander", "Nature", "https://www.ioverlander.com/"],
  ["The Dyrt", "Nature", "https://thedyrt.com/"],
  ["BLM", "Nature", "https://www.blm.gov/"],
  ["US Forest Service", "Nature", "https://www.fs.usda.gov/"],
  ["Recreation.gov", "Nature", "https://www.recreation.gov/"],
  ["Hipcamp", "Nature", "https://www.hipcamp.com/"],
  ["Coworker", "Workspaces", "https://www.coworker.com/"],
].map(([name, category, url]) => ({ name, category, url }));

export function Discover({
  records,
  settings,
  onCreate,
  onSelect,
  onRefresh,
  onSettings,
  onImports,
}: {
  records: AtlasRecord[];
  settings: Settings;
  onCreate: (kind: string) => void;
  onSelect: (r: AtlasRecord) => void;
  onRefresh: () => Promise<void>;
  onSettings: () => void;
  onImports: () => void;
}) {
  const [section, setSection] = useState("Library");
  const [matches, setMatches] = useState<any>(null);
  const [matching, setMatching] = useState(false);
  const [category, setCategory] = useState("All");
  const [search, setSearch] = useState("");
  const [compare, setCompare] = useState<string[]>([]);
  const [criteria, setCriteria] = useState("");
  const [custom, setCustom] = useState(false);
  const [editingSite, setEditingSite] = useState<number | null>(null);
  const [manage, setManage] = useState(false);
  const [newSite, setNewSite] = useState({
    name: "",
    url: "",
    category: "Stays",
  });
  const [error, setError] = useState("");
  const providers = settings.websiteLibrary || PROVIDERS;
  const saved = records.filter(
    (r) => r.kind === "quote" || r.kind === "place" || r.kind === "watchlist",
  );
  const quotes = records.filter((r) => r.kind === "quote");
  const visible = providers.filter(
    (p: any) =>
      (category === "All" || p.category === category) &&
      p.name.toLowerCase().includes(search.toLowerCase()),
  );
  const selected = quotes.filter((r) => compare.includes(r.id));
  async function saveSite() {
    try {
      new URL(newSite.url);
      if (!["https:", "http:"].includes(new URL(newSite.url).protocol))
        throw new Error("Website links must use HTTPS or HTTP.");
      await rpc("settings.save", {
        settings: {
          websiteLibrary:
            editingSite === null
              ? [...providers, newSite]
              : providers.map((p: any, i: number) =>
                  i === editingSite ? newSite : p,
                ),
        },
      });
      await onRefresh();
      setCustom(false);
      setEditingSite(null);
      setNewSite({ name: "", url: "", category: "Stays" });
    } catch (e) {
      setError(String(e));
    }
  }
  return (
    <div className="discover-page">
      <div className="page-intro">
        <h1>Discover</h1>
        <button className="text-button" onClick={onImports}>
          Booking Import Center
        </button>
        <p>Your website library, saved places and date-specific offers.</p>
      </div>
      <div className="subnav">
        <div className="segmented">
          {["Library", "Saved places & offers", "Compare"].map((s) => (
            <button
              key={s}
              className={s === section ? "active" : ""}
              onClick={() => setSection(s)}
            >
              {s}
            </button>
          ))}
        </div>
        <span className="spacer" />
        <button className="secondary" onClick={() => onCreate("quote")}>
          <Bookmark size={14} />
          Save an offer
        </button>
      </div>
      {section === "Library" && <PreparedSearch settings={settings} />}
      {section === "Library" && (
        <>
          <div className="search-row">
            <div className="search-field">
              <Search size={17} />
              <input
                aria-label="Search website library"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Find a website…"
              />
            </div>
            <button
              className="text-button"
              onClick={() => {
                setEditingSite(null);
                setCustom(!custom);
                setNewSite({ name: "", url: "", category: "Stays" });
              }}
            >
              <Plus size={15} />
              Add website
            </button>
          </div>
          <button
            className="text-button muted"
            onClick={() => setManage(!manage)}
          >
            {manage ? "Finish editing library" : "Edit website library"}
          </button>
          <div className="filter-chips">
            {[
              "All",
              "Stays",
              "Travel",
              "Experiences",
              "Nature",
              "Workspaces",
            ].map((c) => (
              <button
                key={c}
                className={category === c ? "active" : ""}
                onClick={() => setCategory(c)}
              >
                {c}
              </button>
            ))}
          </div>
          {custom && (
            <div className="inline-form">
              <input
                aria-label="Website name"
                placeholder="Name"
                value={newSite.name}
                onChange={(e) =>
                  setNewSite({ ...newSite, name: e.target.value })
                }
              />
              <input
                aria-label="Website URL"
                placeholder="https://…"
                value={newSite.url}
                onChange={(e) =>
                  setNewSite({ ...newSite, url: e.target.value })
                }
              />
              <select
                aria-label="Website category"
                value={newSite.category}
                onChange={(e) =>
                  setNewSite({ ...newSite, category: e.target.value })
                }
              >
                {["Stays", "Travel", "Experiences", "Nature", "Workspaces"].map(
                  (c) => (
                    <option key={c}>{c}</option>
                  ),
                )}
              </select>
              <button
                className="primary"
                onClick={saveSite}
                disabled={!newSite.name || !newSite.url}
              >
                Save
              </button>
            </div>
          )}
          {error && <div className="error-banner">{error}</div>}
          <div className="criteria-panel">
            <div>
              <h3>Take your criteria with you.</h3>
              <p>
                Directory links open the provider’s site. Availability and
                prices are checked there.
              </p>
            </div>
            <button className="text-button" onClick={onSettings}>
              Edit preferences
              <ArrowUpRight size={14} />
            </button>
            <textarea
              value={criteria}
              onChange={(e) => setCriteria(e.target.value)}
              placeholder="Temporary search criteria: destination, dates, room type, facilities…"
              rows={2}
            />
            <button
              className="secondary"
              onClick={async () => {
                try {
                  await navigator.clipboard.writeText(criteria);
                  setError("Criteria copied.");
                } catch {
                  setError(
                    "Select your criteria and use your device’s copy command.",
                  );
                }
              }}
              disabled={!criteria}
            >
              Copy criteria
            </button>
          </div>
          <div className="provider-grid">
            {visible.map((p: any, i: number) => (
              <div className="provider-wrapper" key={p.url + i}>
                <button
                  className="provider-card"
                  onClick={() =>
                    openExternal(p.url).catch((e) => setError(String(e)))
                  }
                >
                  <span className="provider-initial">{p.name.charAt(0)}</span>
                  <span>
                    <strong>{p.name}</strong>
                    <small>{p.category} · Website directory</small>
                  </span>
                  <ArrowUpRight size={17} />
                </button>
                {manage && (
                  <div className="provider-edit">
                    <button
                      className="text-button"
                      onClick={() => {
                        setEditingSite(providers.indexOf(p));
                        setNewSite({ ...p });
                        setCustom(true);
                      }}
                    >
                      Edit
                    </button>
                    <button
                      className="text-button danger"
                      onClick={async () => {
                        try {
                          await rpc("settings.save", {
                            settings: {
                              websiteLibrary: providers.filter(
                                (_: any, j: number) =>
                                  j !== providers.indexOf(p),
                              ),
                            },
                          });
                          await onRefresh();
                        } catch (e) {
                          setError(String(e));
                        }
                      }}
                    >
                      Remove
                    </button>
                  </div>
                )}
              </div>
            ))}
          </div>
          <p className="footnote">
            Prepared website links · No live feed connected · Saved quotes
            retain their source and checked date
          </p>
        </>
      )}
      {section === "Saved places & offers" && (
        <>
          <div className="section-heading">
            <h2>Your shortlist</h2>
            <div className="inline-actions">
              <button
                className="secondary"
                onClick={() => onCreate("watchlist")}
              >
                Add watchlist item
              </button>
              <button
                className="secondary"
                disabled={matching || quotes.length === 0}
                onClick={async () => {
                  setMatching(true);
                  try {
                    setMatches(await rpc("discovery.rank"));
                    setError("");
                  } catch (e) {
                    setError(String(e));
                  } finally {
                    setMatching(false);
                  }
                }}
              >
                {matching ? "Comparing…" : "Match my preferences"}
              </button>
              <button className="text-button" onClick={() => onCreate("place")}>
                <Plus size={14} />
                Save a place
              </button>
            </div>
          </div>
          {error && <div className="error-banner">{error}</div>}
          {matches && (
            <section className="match-results">
              <span className="eyebrow">YOUR PREFERENCES, APPLIED</span>
              <h3>Compare like with like.</h3>
              <p className="field-help">
                Offers are ranked only within matching dates and currency, using
                saved evidence. Unknown facilities do not meet a requirement. No
                live search was performed.
              </p>
              {matches.groups?.map((group: any, i: number) => (
                <div className="match-group" key={i}>
                  <div className="section-heading">
                    <strong>
                      {group.checkIn} → {group.checkOut}
                    </strong>
                    <span className="muted">
                      {group.nights} nights · {group.currency}
                    </span>
                  </div>
                  <p className="field-help">
                    Date-specific target{" "}
                    {money(group.targetForDatesMinor, group.currency)} · stretch
                    cap {money(group.stretchForDatesMinor, group.currency)}
                  </p>
                  {group.offers.map((offer: any) => (
                    <article className="match-offer" key={offer.id}>
                      <button
                        className="offer-name"
                        onClick={() => onSelect(offer.record)}
                      >
                        {offer.rankWithinGroup}. {offer.name}
                      </button>
                      <div className="inline-actions">
                        <strong>
                          {money(offer.knownPayableMinor, group.currency)}
                        </strong>
                        <span className="status-badge">
                          {offer.affordability}
                        </span>
                        <span className="status-badge">
                          {offer.requirementsVerified
                            ? "Requirements verified"
                            : "Requirements unverified"}
                        </span>
                      </div>
                      <ul className="match-reasons">
                        {offer.reasons?.map((reason: string, j: number) => (
                          <li key={j}>{reason}</li>
                        ))}
                      </ul>
                      {offer.stretchReason && (
                        <p className="field-help">
                          Stretch reason: {offer.stretchReason}
                        </p>
                      )}
                      <p className="field-help">
                        {offer.source?.provider || "Saved source"} ·{" "}
                        {offer.source?.checkedAt
                          ? "Checked " + offer.source.checkedAt
                          : "Freshness unknown"}
                        {offer.feesComplete
                          ? " · known fees included"
                          : " · fees incomplete"}
                      </p>
                    </article>
                  ))}
                </div>
              ))}
              {matches.skipped?.length > 0 && (
                <p className="field-help">
                  Excluded from comparison:{" "}
                  {matches.skipped
                    .map((s: any) => `${s.name}: ${s.reason}`)
                    .join("; ")}
                </p>
              )}
            </section>
          )}
          {saved.length === 0 ? (
            <div className="empty">
              <Bookmark size={32} />
              <h3>Keep the good finds.</h3>
              <p>
                Save a place or a date-specific quote. Alternatives stay out of
                your committed budget.
              </p>
              <button
                className="secondary"
                onClick={() => onCreate("watchlist")}
              >
                Add watchlist item
              </button>
              <button className="primary" onClick={() => onCreate("quote")}>
                Save your first offer
              </button>
            </div>
          ) : (
            <div className="offer-grid">
              {saved.map((r) => (
                <article className="offer-card" key={r.id}>
                  <div className="offer-art">
                    <MapPin size={30} />
                    <span>{r.type || r.category || "Saved place"}</span>
                  </div>
                  <div className="offer-content">
                    <div className="record-label">
                      <span>{r.provider || "Saved by you"}</span>
                      <small>
                        {r.checkedAt
                          ? "Checked " + r.checkedAt
                          : "Freshness unknown"}
                      </small>
                    </div>
                    <button className="offer-name" onClick={() => onSelect(r)}>
                      {r.name}
                    </button>
                    <p>{r.address || "Location not recorded"}</p>
                    {r.kind === "quote" && (
                      <>
                        <div className="offer-total">
                          <strong>{money(r.totalMinor, r.currency)}</strong>
                          <span>
                            {r.checkIn && r.checkOut
                              ? nights(r.checkIn, r.checkOut) + " nights"
                              : "Date-specific quote"}
                          </span>
                        </div>
                        <p className="field-help">
                          {r.feesComplete
                            ? "Known mandatory fees included"
                            : "Total may exclude unknown fees and taxes"}
                        </p>
                        <label className="check-field">
                          <input
                            type="checkbox"
                            checked={compare.includes(r.id)}
                            onChange={(e) =>
                              setCompare(
                                e.target.checked
                                  ? [...compare, r.id]
                                  : compare.filter((id) => id !== r.id),
                              )
                            }
                          />
                          Compare this offer
                        </label>
                      </>
                    )}
                    <WatchlistActions record={r} onRefresh={onRefresh} />
                  </div>
                </article>
              ))}
            </div>
          )}
        </>
      )}
      {section === "Compare" && (
        <OfferComparison
          ids={compare}
          records={records}
          settings={settings}
          onSelect={onSelect}
        />
      )}
    </div>
  );
}
function facility(r: AtlasRecord, key: string) {
  return r.facilities?.[key] === true
    ? "Verified available"
    : r.facilities?.[key] === false
      ? "Not available"
      : "Unknown";
}

export function MapView({
  records,
  onSelect,
  onCreate,
}: {
  records: AtlasRecord[];
  onSelect: (r: AtlasRecord) => void;
  onCreate: (kind: string) => void;
}) {
  const places = records.filter(
    (r) =>
      ["stay", "place", "objective"].includes(r.kind) &&
      (r.address || Number.isFinite(r.latitude)),
  );
  return (
    <div className="map-page">
      <div className="page-intro">
        <h1>Map</h1>
        <p>
          Your saved locations, ready for directions. Open a mapping provider
          for live routes.
        </p>
      </div>
      <div className="map-layout">
        <div className="map-illustration">
          <div className="map-grid" />
          <div className="map-compass">
            <Compass size={68} strokeWidth={1} />
            <h2>A sense of place.</h2>
            <p>
              {places.length} saved location{places.length === 1 ? "" : "s"}
            </p>
            <small>Location directory · map tiles are not downloaded</small>
          </div>
        </div>
        <div className="map-place-list">
          <div className="section-heading">
            <h3>Your places</h3>
            <button
              className="icon-button"
              aria-label="Add place"
              onClick={() => onCreate("place")}
            >
              <Plus size={17} />
            </button>
          </div>
          {places.length === 0 ? (
            <p className="muted">
              Add an address to a stay or save a place to begin.
            </p>
          ) : (
            places.map((r, i) => (
              <div className="map-place" key={r.id}>
                <span className="map-pin-number">{i + 1}</span>
                <button onClick={() => onSelect(r)}>
                  <strong>{r.name}</strong>
                  <small>{r.address || `${r.latitude}, ${r.longitude}`}</small>
                </button>
                <button
                  className="icon-button"
                  title="Open directions"
                  aria-label={`Open map for ${r.name}`}
                  onClick={() =>
                    openExternal(
                      `https://www.google.com/maps/search/?api=1&query=${encodeURIComponent(r.address || `${r.latitude},${r.longitude}`)}`,
                    )
                  }
                >
                  <ArrowUpRight size={16} />
                </button>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}

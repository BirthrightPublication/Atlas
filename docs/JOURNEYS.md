# Journeys, route drafts and offline preparation

Open **Map → Journey planning & offline trip pack**. Add up to 40 stops within a
366-day planning range. Enter chronological arrival/departure dates, IANA zones,
On Foot/On Road context, known accommodation cost and an optional daily spending
estimate. Leave unknown amounts empty. Link saved stays and incoming transport.
The costs are original-currency integer minor units, allocated through the real
accounting services across the actual occupied nights. Unknown amounts are not
zero-cost verified offers. Cross-currency totals are not silently converted.

Compare a draft with the saved plan before saving it. Reordering and date edits
apply to the tentative stop list. A linked confirmed stay remains unchanged;
conflicting dates, missing accommodation, transport and connectivity are shown
as issues. A saved journey does not create reservations or settle payments.

The scheduling tool proposes up to 31 days using destination-local and employer
local times, DST-aware IANA rules, saved protected work and required work-window
suggestions. It avoids saved transport, activities and checkout buffers. Each
objective is accepted, renamed or rejected individually. Accepting a stale slot
fails if current work or activities now conflict. Proposals persist separately
from the itinerary. Attraction hours, accessibility and travel time are not
assumed. Work requirements stay configurable, defaulting to an eight-hour day
with four hours of overlap in America/Los_Angeles.

Route drafts link two saved places for driving, walking, transit or long-distance
travel. They save notes, the selected date/context and a provider directions
link. Google Maps/Rome2Rio supplies external routing; Atlas has no offline
routing engine and does not invent duration, distance or availability.

Save a journey before exporting an offline package. Its JSON includes selected
saved itinerary records, addresses/instructions/contacts, reminders, work rules
and explicitly chosen document contents. It excludes mailbox connections and
raw import sources. Limit packages to ten chosen documents and the stated size
cap. Budget entries are whole-month saved calculations, not isolated per-trip
spending. The desktop data already works offline.

On the paired companion, select **Trip** after synchronizing. Linked stay and
transport information, destination notes and work requirements are readable
without a connection. Keep individual documents in **Documents → Keep offline**;
choosing a trip does not copy all imported documents. Reconnect to refresh cached
budget information. Physical iPhone/private HTTPS validation remains an owner
check; Chromium offline tests do not establish it.

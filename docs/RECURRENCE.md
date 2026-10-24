# Local calendar recurrence

Objective and work-block templates may set `recurrence` to `daily`, `weekly`,
or `monthly`. Advanced callers may use `{frequency, interval, weekdays}`;
weekly weekdays are ISO Monday=1 through Sunday=7 or weekday names. The start
date anchors the series. Weekly intervals count Monday-based local calendar
weeks. A monthly series preserves its original day number and skips months
without that day; a January 31 series next occurs March 31.

`recurrenceUntil` is inclusive. `recurrenceExceptions` is a list of local
YYYY-MM-DD dates to skip. `recurrenceCompletedDates` marks individual
objectives done. `completedOccurrences` is accepted as a compatibility alias.
An empty recurrence string removes recurrence.

`calendar.occurrences` expands a range with an inclusive `start` and exclusive
`end`. The range must span 1–366 days. Nonrecurring records are returned
unchanged; callers select the kinds/dates relevant to their view. An occurrence
keeps its template `id` and also has `occurrenceId` (`id:date`), `occurrenceDate`,
and `recurrenceParentId`. Use `occurrenceId` for display keys and the original
template record for series editing; do not save an expanded occurrence over
its template. Expenses for one recurring objective should include
`objectiveId` and `occurrenceDate`.

Timed recurrence uses local calendar dates and the named IANA timezone.
Actual instants are recomputed for every occurrence, so 09:00 remains 09:00
across daylight-saving changes. Ambiguous and nonexistent local times produce
a precise review error; set an explicit time or add an exception for that date.
The app never silently guesses which repeated clock time was intended.

Imported RRULE supports FREQ, INTERVAL, date-only UNTIL, and weekly BYDAY.
Unsupported RRULE components require review rather than silently changing the
schedule. Complex nth-weekday, exclusion-rule, and counted recurrences are not
implemented.

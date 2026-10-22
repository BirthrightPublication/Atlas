# Version 0.3 validation

This report will be finalized with the tested source and native CI results.
All fixtures use synthetic travel data and isolated profiles.

Local checks: frontend build, core/service regression tests, persistent chats,
notification state, work/gap/scenario calculations and five viewport browser
journeys. Controlled HTTPS validation accepts its disposable CA and rejects the
same endpoint without that CA; TLS verification is enabled.

Native Windows installer upgrade and signed-update installation are pending CI.
Production signing credentials and a published stable feed are not configured.
Live Proton authorization, physical iPhone and optional model inference remain
separate unverified external checks, as documented in earlier versions.

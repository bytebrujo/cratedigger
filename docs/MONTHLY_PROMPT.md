Run the monthly maintenance pass for this repo:
1. cargo update; upgrade the MCP SDK if a new version exists (read its changelog
   first); keep make check green.
2. Run the live smoke locally; fix any upstream drift.
3. Address open issues labeled maintenance.
4. If any user-visible change occurred, update CHANGELOG and cut a release tag.
5. Leave CI green on main.
6. Append a MAINTENANCE_LOG.md entry using the template. Do not edit PREDICTIONS.md.

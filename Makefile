.PHONY: ingest

ingest:
	@test -n "$(RESULTS_DIR)" || { echo 'usage: make ingest RESULTS_DIR=/path/to/results' >&2; exit 2; }
	node scripts/ingest.mjs "$(RESULTS_DIR)"

.PHONY: demo demo-down ingest

-include .env
export METRICS_OPERATOR_TOKEN PROJECTBENCH_API_PORT PROJECTBENCH_WEB_PORT PROJECTBENCH_POSTGRES_PORT

DEMO_PROJECT ?= projectbench-demo
METRICS_OPERATOR_TOKEN ?= projectbench-demo-token
PROJECTBENCH_API_PORT ?= 18080
PROJECTBENCH_WEB_PORT ?= 4174
PROJECTBENCH_POSTGRES_PORT ?= 15432

demo:
	@set -eu; \
	  export METRICS_OPERATOR_TOKEN="$(METRICS_OPERATOR_TOKEN)"; \
	  export PROJECTBENCH_API_PORT="$(PROJECTBENCH_API_PORT)"; \
	  export PROJECTBENCH_WEB_PORT="$(PROJECTBENCH_WEB_PORT)"; \
	  export PROJECTBENCH_POSTGRES_PORT="$(PROJECTBENCH_POSTGRES_PORT)"; \
	  docker compose -p "$(DEMO_PROJECT)" --profile demo build api web demo-app; \
	  export DEMO_IMAGE_DIGEST="$$(docker image inspect projectbench-demo:local --format='{{.Id}}')"; \
	  export DEMO_COMMIT="$$(git rev-parse HEAD 2>/dev/null || echo local-source)"; \
	  if [ "$$(uname -s)" = Darwin ]; then export DEMO_HOST_CPU="$$(sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -m)"; \
	  elif command -v lscpu >/dev/null 2>&1; then export DEMO_HOST_CPU="$$(lscpu | awk -F: '/Model name/ {gsub(/^[[:space:]]+/, "", $$2); print $$2; exit}')"; \
	  else export DEMO_HOST_CPU="$$(uname -m)"; fi; \
	  export DEMO_OS="$$(uname -sr)"; \
	  export DEMO_DOCKER_VERSION="$$(docker version --format '{{.Server.Version}}')"; \
	  docker compose -p "$(DEMO_PROJECT)" --profile demo up -d postgres api web demo-app; \
	  docker compose -p "$(DEMO_PROJECT)" --profile demo run --rm demo-runner

demo-down:
	docker compose -p "$(DEMO_PROJECT)" --profile demo down

ingest:
	@test -n "$(RESULTS_DIR)" || { echo 'usage: make ingest RESULTS_DIR=/path/to/results' >&2; exit 2; }
	node scripts/ingest.mjs "$(RESULTS_DIR)"

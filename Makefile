SHELL := /bin/bash

.PHONY: bootstrap bootstrap-agents build test ui-test check bench governance-check docs docs-install docs-build docs-check

bootstrap:
	@bash scripts/task.sh bootstrap

bootstrap-agents:
	@bash scripts/task.sh bootstrap-agents

build:
	@bash scripts/task.sh build

test:
	@bash scripts/task.sh test

ui-test:
	@bash scripts/task.sh ui-test

check:
	@bash scripts/task.sh check

bench:
	@bash scripts/task.sh bench

governance-check:
	@bash scripts/validate-governance.sh

docs-install:
	@npm --prefix site ci --no-audit --no-fund

docs: docs-install
	@npm --prefix site run dev

docs-build: docs-install
	@npm --prefix site run build

docs-check: docs-install
	@npm --prefix site run check

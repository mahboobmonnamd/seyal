SHELL := /bin/bash

.PHONY: bootstrap bootstrap-agents build test check bench governance-check

bootstrap:
	@bash scripts/task.sh bootstrap

bootstrap-agents:
	@bash scripts/task.sh bootstrap-agents

build:
	@bash scripts/task.sh build

test:
	@bash scripts/task.sh test

check:
	@bash scripts/task.sh check

bench:
	@bash scripts/task.sh bench

governance-check:
	@bash scripts/validate-governance.sh

SHELL := /bin/bash

.PHONY: bootstrap build test check bench governance-check

bootstrap:
	@./scripts/task.sh bootstrap

build:
	@./scripts/task.sh build

test:
	@./scripts/task.sh test

check:
	@./scripts/task.sh check

bench:
	@./scripts/task.sh bench

governance-check:
	@./scripts/validate-governance.sh

SHELL := /bin/bash

.PHONY: doctor check fmt lint test repo-test protocol-test agent-test api-test chain-test chain-v2-check chain-v2-test web-test web-e2e firmware-build firmware-test infra-up infra-down vectors spec-check bootstrap-program-id bootstrap-program-id-v2 local-demo-doctor local-demo-init local-demo-up local-demo-status local-demo-test local-demo-test-headed local-demo-down local-demo-reset

doctor:
	python3 scripts/doctor.py

fmt:
	cargo fmt --all -- --check
	npm --workspace @lastro/web run format:check

lint:
	cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
	npm --workspace @lastro/web run typecheck

protocol-test:
	cargo test --locked -p lastro-protocol

agent-test:
	cargo test --locked -p lastro-agent

api-test:
	cargo test --locked -p lastro-api

bootstrap-program-id:
	scripts/bootstrap_program_id.sh

bootstrap-program-id-v2:
	scripts/bootstrap_program_id_v2.sh verify

chain-test:
	@grep -Eq '^[[:space:]]*declare_id!\(' chain/programs/lastro/src/lib.rs || { echo "Run make bootstrap-program-id first" >&2; exit 1; }
	cd chain && anchor test

chain-v2-check:
	cargo check --offline --manifest-path chain/Cargo.toml -p lastro-v2

chain-v2-test:
	cargo test --locked --manifest-path chain/Cargo.toml -p lastro-v2

web-test:
	npm --workspace @lastro/web run test

web-e2e:
	npm --workspace @lastro/web run test:e2e

firmware-build:
	idf.py -C firmware/station set-target esp32c5
	idf.py -C firmware/station build

firmware-test:
	@echo "Run target Unity tests on the ESP32-C5 test app; see docs/TESTING.md."

infra-up:
	docker compose -f infra/compose.yml up -d postgres

infra-down:
	docker compose -f infra/compose.yml down

local-demo-doctor:
	python3 scripts/local_dev.py doctor

local-demo-init:
	python3 scripts/local_dev.py init

local-demo-up:
	python3 scripts/local_dev.py up

local-demo-status:
	python3 scripts/local_dev.py status

local-demo-test:
	python3 scripts/local_dev.py test

local-demo-test-headed:
	python3 scripts/local_dev.py test --headed

local-demo-down:
	python3 scripts/local_dev.py down

local-demo-reset:
	python3 scripts/local_dev.py reset

vectors:
	python3 scripts/check_vectors.py --verify-only

repo-test:
	python3 -m pytest -q tests/repository

spec-check:
	python3 scripts/spec_check.py

check: spec-check repo-test fmt lint
	cargo test --locked --workspace
	npm --workspace @lastro/web run test

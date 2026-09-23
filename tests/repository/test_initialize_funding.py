"""Finalized funding is required before ProtocolConfig initialization."""

from __future__ import annotations

import pytest

from scripts.initialize_protocol_config import wait_for_finalized_funding
from tests.system.support import SystemContractError


class FakeRpc:
    def __init__(self, balances: list[int]) -> None:
        self.balances = iter(balances)
        self.calls: list[tuple[str, list[object]]] = []

    def call(self, method: str, params: list[object]) -> dict[str, int]:
        self.calls.append((method, params))
        return {"value": next(self.balances)}


def test_initializer_waits_for_the_finalized_bank_to_credit_authority() -> None:
    rpc = FakeRpc([0, 1_000_000])
    wait_for_finalized_funding(rpc, "authority", timeout=5)
    assert rpc.calls == [
        ("getBalance", ["authority", {"commitment": "finalized"}]),
        ("getBalance", ["authority", {"commitment": "finalized"}]),
    ]


def test_initializer_rejects_unfunded_authority_without_sending_a_transaction() -> None:
    rpc = FakeRpc([])
    with pytest.raises(SystemContractError, match="funding did not finalize"):
        wait_for_finalized_funding(rpc, "authority", timeout=0)
    assert rpc.calls == []

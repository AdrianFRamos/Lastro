from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def test_core_runtime_source_does_not_depend_on_hardware_simulator():
    core_roots = [ROOT / name for name in ("apps", "services", "crates", "chain", "firmware")]
    if not all(path.exists() for path in core_roots):
        return

    findings: list[str] = []
    text_suffixes = {".c", ".h", ".rs", ".ts", ".vue", ".py", ".toml"}
    for base in core_roots:
        for path in base.rglob("*"):
            if not path.is_file() or path.suffix not in text_suffixes:
                continue
            text = path.read_text(encoding="utf-8", errors="ignore")
            if "lastro_hardware_simulator" in text or "hardware-simulator" in text:
                findings.append(path.relative_to(ROOT).as_posix())
    assert findings == [], "production source must not depend on simulator: " + ", ".join(findings)


def test_simulator_source_stops_at_station_agent_boundary():
    source = ROOT / "hardware-simulator" / "src"
    if not source.exists():
        source = Path(__file__).resolve().parents[1] / "src"
    combined = "\n".join(
        path.read_text(encoding="utf-8", errors="ignore")
        for path in source.rglob("*.py")
    ).lower()
    assert "lastro_database_url" not in combined
    assert "solana_rpc" not in combined
    assert "postgres" not in combined
    assert "/api/captures" not in combined
    assert "/api/agent" not in combined


def test_compose_keeps_simulator_optional_and_wire_private():
    compose_path = ROOT / "infra" / "compose.yml"
    if not compose_path.exists():
        return
    text = compose_path.read_text(encoding="utf-8")
    simulator_block = text.split("  hardware-simulator:\n", 1)[1].split("  hardware-simulator-agent:\n", 1)[0]
    agent_block = text.split("  hardware-simulator-agent:\n", 1)[1].split("\nvolumes:\n", 1)[0]
    assert 'profiles: ["hardware-sim"]' in simulator_block
    assert 'profiles: ["hardware-sim"]' in agent_block
    assert '"127.0.0.1:8090:8090"' in simulator_block
    assert '"9100:9100"' not in simulator_block
    assert 'LASTRO_AGENT_SERIAL_PORT: /tmp/lastro-station' in agent_block

    production_prefix = text.split("  hardware-simulator:\n", 1)[0]
    assert "hardware-simulator" not in production_prefix

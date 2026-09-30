from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]

def test_envelope_length_is_consistent_across_languages_and_docs():
    files=[
        ROOT/'docs/PROTOCOL.md', ROOT/'crates/lastro-protocol/src/v2/constants.rs',
        ROOT/'firmware/station/components/lastro_station/include/lastro_station/event.h',
        ROOT/'apps/web/src/protocol/v2/domainEvent.ts'
    ]
    for f in files:
        assert '220' in f.read_text(), f

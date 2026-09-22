from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]

def test_station_event_length_is_consistent_across_languages_and_docs():
    files=[
        ROOT/'docs/PROTOCOL.md', ROOT/'crates/lastro-protocol/src/constants.rs',
        ROOT/'firmware/station/components/lastro_station/include/lastro_station/event.h',
        ROOT/'apps/web/src/protocol/constants.ts'
    ]
    for f in files:
        assert '276' in f.read_text(), f

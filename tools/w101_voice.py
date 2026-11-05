"""Copy the street NPCs' classic quest voice lines out of the Wizard101 install for the HUD DLL (w101_orbs\\voice).

Which files: the keys of w101_npc_dialogue.VOICE (round 23/23b). Wizard101 streams voice on demand, so a file is
only there (not all zeros) after the client has played it: play the Unicorn Way quest line up to Rattlebones in
Classic Mode once. Files are searched in every .wad of GameData (they sit in _Shared-WorldData.wad,
Sound_Dialogue-WorldData.wad and Sound_Dialogue_GEN-WorldData.wad). Never committed.

Usage: python -I w101_voice.py <Wizard101\\Data\\GameData> <orbs\\w101_orbs\\voice>
"""
import sys
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from w101_npc_dialogue import VOICE  # noqa: E402
from w101_sfx import entries  # noqa: E402


def main(argv):
    data, out = Path(argv[1]), Path(argv[2])
    out.mkdir(parents=True, exist_ok=True)
    want = set(VOICE)
    empty = set()
    for wad in sorted(data.glob("*.wad")):
        try:
            listing = list(entries(wad))
        except (AssertionError, OSError):
            continue
        with open(wad, "rb") as f:
            for name, offset, size, stored, compressed in listing:
                base = name.rsplit("/", 1)[-1]
                if base not in want:
                    continue
                f.seek(offset)
                raw = f.read(stored if compressed else size)
                if not raw.strip(b"\0"):
                    empty.add(base)
                    continue
                (out / base).write_bytes(zlib.decompress(raw) if compressed else raw)
                want.discard(base)
                empty.discard(base)
                print("wrote", base, "from", wad.name)
    for base in sorted(want):
        print("not downloaded yet (play that line in Classic Mode):" if base in empty else "missing:", base)
    print(f"{len(VOICE) - len(want)} of {len(VOICE)} voice files copied")


if __name__ == "__main__":
    main(sys.argv)

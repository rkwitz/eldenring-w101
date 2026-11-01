"""Copies Wizard101's classic Unicorn Way music out of the Wizard101 install for the HUD DLL (round 20 M1-M4).

Usage: python w101_music.py path\\to\\Wizard101\\Data\\GameData path\\to\\orbs\\w101_orbs\\music

Which tracks (found in the install, see hud_orbs/src/music.rs):
- Unicorn Way's zone (WizardCity-WC_Streets-WC_Unicorn.wad gamedata.bin) plays Sound/WizardCity4M.mp3 on the street
  and, since the zone was updated, WC/WC_HauntedWay_Combat.ogg in combat.
- Classic mode (ClassicMode-WorldData.wad, the same files as Music-WorldData.wad WC_Original/) has the classic
  WizardCity4M.mp3 and the classic Wizard City combat track Combat1Test.mp3 ("Wizard City Combat Theme" in
  WizardFurniture.lang). Wizard City has no boss track: its boss rooms use the combat track.

Wizard101 streams music it hasn't played yet, so a WAD entry can be all zeros: such a track is skipped and reported.
Play a fight in Wizard101 with Classic Mode on (any Wizard City street) once, then run this again.
"""
import os
import struct
import sys
import zlib

# (output name, [(wad, entry), ...] tried in order)
TRACKS = [
    ("WizardCity4M_classic.mp3", [("ClassicMode-WorldData.wad", "Music/WizardCity4M.mp3"),
                                  ("Music-WorldData.wad", "WC_Original/WizardCity4M.mp3")]),
    ("Combat1Test_classic.mp3", [("ClassicMode-WorldData.wad", "Music/Combat1Test.mp3"),
                                 ("Music-WorldData.wad", "WC_Original/Combat1Test.mp3")]),
]


def wad_entry(path, wanted):
    with open(path, "rb") as f:
        assert f.read(5) == b"KIWAD", f"{path} is not a Wizard101 WAD"
        version, count = struct.unpack("<II", f.read(8))
        if version >= 2:
            f.read(1)
        for _ in range(count):
            offset, size, zsize, zipped, _crc, name_len = struct.unpack("<IIIBII", f.read(21))
            name = f.read(name_len).rstrip(b"\0").decode("latin-1")
            if name == wanted:
                f.seek(offset)
                data = f.read(zsize if zipped else size)
                return zlib.decompress(data) if zipped else data
    return None


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    game_data, out = sys.argv[1], sys.argv[2]
    os.makedirs(out, exist_ok=True)
    missing = 0
    for name, sources in TRACKS:
        for wad, entry in sources:
            data = wad_entry(os.path.join(game_data, wad), entry)
            if data is None:
                continue
            # Not streamed in yet: Wizard101 keeps the entry's room filled with zeros.
            if data[:4096].count(0) > 4000 or data.count(0) > len(data) * 0.5:
                print(f"{wad} {entry}: not downloaded yet (all zeros)")
                continue
            with open(os.path.join(out, name), "wb") as f:
                f.write(data)
            print(f"{name}: {len(data)} bytes from {wad} {entry}")
            break
        else:
            missing += 1
            print(f"{name}: MISSING - play a Classic Mode fight in Wizard101 once, then run this again")
    sys.exit(1 if missing else 0)


if __name__ == "__main__":
    main()

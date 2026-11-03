"""Wizard101's interface sounds for the HUD DLL (round 22 S3): copies the Sound/GUI .wav files the DLL plays
(hud_orbs/src/music.rs `sfx`) out of the Wizard101 install into w101_orbs\\sfx\\ on Ryan's PC. Never committed.

Which sound goes where comes from the .gui files' m_sSelectSound / m_sHoverSound (_Shared-WorldData.wad
GUI/DialogWindow.gui: btnRight HudSelect, btnBack CancelButtonClick, btnExit HudCancel; GUI/DeckConfiguration.gui:
PageUp / PageDown ui_hudgenericnext_click, school tabs ui_hudspellbook_schoolswitch) and the sound names themselves
(ui_hudspellbook_open / _close, ui_book_pageturn, ui_quest_accepted / _decline / _updated, ui_reward_gold, ...).

Usage: python -I w101_sfx.py <Wizard101\\Data\\GameData> <orbs\\w101_orbs\\sfx>
"""
import struct
import sys
import zlib
from pathlib import Path

SOUNDS = {
    "Root.wad": ["HudSelect", "HudMouseOver", "HudCancel", "CancelButtonClick", "OKButtonClick", "ButtonMouseOver",
                 "ui_hudspellbook_open", "ui_hudgenericnext_click", "ui_hudgenericnext_hover", "ui_chatbubble_popup"],
    "_Shared-WorldData.wad": ["ui_book_open", "ui_book_close", "ui_book_pageturn", "ui_bookmark_clickopen",
                              "ui_hudspellbook_close", "ui_hudspellbook_schoolswitch", "ui_hudmainbutton_click",
                              "ui_hudmainbutton_hover", "ui_quest_accepted", "ui_quest_decline", "ui_quest_updated",
                              "ui_reward_gold", "ui_reward_xp", "ui_reward_item", "ui_button_press",
                              "ui_button_highlight", "ui_teleport_marker", "ui_homebutton_select", "ui_chatbubble_female",
                              "ui_chatbubble_male"],
}


# Round 31 (SND1): the "Original" (default) teleport emotes' sounds, Root.wad ObjectData/Emotes/
# Teleport43..49_Original_<School>_01.xml -> _Shared-WorldData.wad Sound/PlayerEmotes/Teleports/.
TELEPORTS = [f"Spawner_Special_{school}_{way}.ogg" for school in ("Balance", "Death", "Fire", "Ice", "Life", "Myth", "Storm")
             for way in ("OUT", "IN")]


def entries(path):
    """KIWAD entries: (name, offset, size, stored size, compressed)."""
    with open(path, "rb") as f:
        assert f.read(5) == b"KIWAD"
        version, count = struct.unpack("<II", f.read(8))
        if version >= 2:
            f.read(1)
        for _ in range(count):
            offset, size, stored, compressed, _crc, n = struct.unpack("<IIIBII", f.read(21))
            yield f.read(n).rstrip(b"\0").decode("latin-1"), offset, size, stored, compressed


def main(argv):
    data, out = Path(argv[1]), Path(argv[2])
    out.mkdir(parents=True, exist_ok=True)
    for wad, names in SOUNDS.items():
        want = {f"Sound/GUI/{n}.wav" for n in names}
        if wad == "_Shared-WorldData.wad":
            want |= {f"Sound/PlayerEmotes/Teleports/{n}" for n in TELEPORTS}
        with open(data / wad, "rb") as f:
            for name, offset, size, stored, compressed in entries(data / wad):
                if name not in want:
                    continue
                f.seek(offset)
                raw = f.read(stored if compressed else size)
                if not raw.strip(b"\0"):
                    print("not downloaded:", wad, name)
                    continue
                (out / Path(name).name).write_bytes(zlib.decompress(raw) if compressed else raw)
                want.discard(name)
                print("wrote", Path(name).name)
        for name in sorted(want):
            print("missing:", wad, name)


if __name__ == "__main__":
    main(sys.argv)

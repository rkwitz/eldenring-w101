"""Delete named parts from an MSB, remapping every part index (dome_clear.delete_parts, as msb_audit.py deletes).

Round 21 (Ryan: "The Seraph is Malenia. Just get rid of that."): the Seraph boss of msb_add_boss.py is Malenia
(c2120_9000, entity 1042360853) in the hedge maze's north alcove; it goes. Its arena/sigil regions 1042362828/829
stay (only its own events in m60_42_36_00.emevd use them). Its events there (bar 1042362822, phase one 1042362823,
the Seraph lines of boss setup 1042362814) only wait on or act on the entity, so they do nothing once it is gone.

Usage: python msb_remove_parts.py in.msb.dcx out.msb.dcx part_name...
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from dome_clear import delete_parts, load  # noqa: E402
from msb_add_ghost import check_grouped  # noqa: E402
from msb_audit import dflt  # noqa: E402
from msb_floor import _list, entry_name, read_msb, write_msb  # noqa: E402

SERAPH = "c2120_9000"


def remove(raw, names):
    header, lists = read_msb(raw)
    parts = _list(lists, "PARTS_PARAM_ST").entries
    found = {entry_name(e): k for k, e in enumerate(parts)}
    missing = [n for n in names if n not in found]
    assert not missing, f"not in the map: {missing}"
    lines = [f"part {n} (index {found[n]}) deleted" for n in names]
    delete_parts("map", lists, {found[n] for n in names}, lines)
    check_grouped(lists)
    return write_msb(header, lists), lines


def main():
    src, out, *names = sys.argv[1:]
    template = Path(src).read_bytes()
    new, lines = remove(load(src), names or [SERAPH])
    print("\n".join(lines))
    Path(out).write_bytes(dflt(new, template))


if __name__ == "__main__":
    main()

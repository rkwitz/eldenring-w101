"""Round 26 (W3): make a character's animations never fade it out (TAE opacity events), for Rattlebones' c4960.

Why (Ryan's 21:44 log: Rattlebones' HP stays 7771 and his position stays put, 0.2 m over the floor, for 40 s; he is
there but unseen): c4960.anibnd.dcx's c4960.tae (54 animations) is built as a phantom. TAE event type 193 is an
opacity keyframe (f32 opacity at the event's start, f32 at its end), and:
  - idle 20, walk/turn 2000/2001, hit reactions 5000-5003, 3020/3021, 7000/7400, 30015: opacity 0 for the whole
    animation (the base set also holds him 4.6 m under the floor: head bone y -4.6 in a000_000020 / 002000);
  - attacks 3000/3002/3004/3005: fade in 0 -> 1 at the start, out 1 -> 0 at the end;
  - 10000 and 20015 (collapse onto the floor, head y 1.0 at the end): 1 -> 0, then 0 until the end;
  - 20010/20011/20012 (rise out of the floor, applying SpEffect 5403 = animIdOffset 3000000, the risen set): 0 -> 1.
NpcParam 49600168 (our row) starts him with 5403; the risen set (a003_*: head y 5.34) has no idle-walk of its own
(no 3002000/3002001/3020015), so those fall back to the invisible base ones. So he can drop out of sight with HP and
position unchanged, and only a rise animation (20010-20012) brings him back; vanilla's m12_05 event 12052250 plays
20010 from outside, our boss_bar only on walking in (which is why the 21:51 fight, after Ryan walked back in, was
normal).
This sets every type-193 event's two opacities to 1.0 in place (same size: the binder and its offsets don't move),
so no animation can make him transparent; everything else (movement, hit boxes, timing) is untouched.

Usage (container check, the Blender step does the real write with the game's compression):
  python er_chr_opaque.py c4960.anibnd.dcx   -> prints the events it would change and checks the result
"""
import struct
import sys
from pathlib import Path

OPACITY = 193


def _tae_events(b):
    """(animation id, event type, params offset) of every event of a TAE (Elden Ring 64-bit layout)."""
    assert b[:4] == b"TAE ", "not a TAE"
    q = lambda o: struct.unpack_from("<q", b, o)[0]  # noqa: E731
    i = lambda o: struct.unpack_from("<i", b, o)[0]  # noqa: E731
    o = 0x10 + 8 * 4 + 8 * 4
    count, anims = i(o + 4), q(o + 8)
    out = []
    for k in range(count):
        aid, off = q(anims + 16 * k), q(anims + 16 * k + 8)
        heads, n = q(off), i(off + 32)
        for e in range(n):
            data = q(heads + 24 * e + 16)
            out.append((aid, i(data), q(data + 8)))
    return out


def opaque(binder):
    """`binder` (a decompressed anibnd BND4) with every TAE opacity event set to 1 -> 1; and the list of changes."""
    from er_text import bnd4
    data = bytearray(binder)
    changes = []
    for name, tae in bnd4(bytes(binder)).items():
        if not name.lower().endswith(".tae"):
            continue
        base = bytes(binder).find(tae)
        assert base > 0 and bytes(binder).find(tae, base + 1) < 0, f"{name}: not found once in the binder"
        for aid, kind, params in _tae_events(tae):
            if kind != OPACITY or not params:
                continue
            old = struct.unpack_from("<2f", tae, params)
            if old != (1.0, 1.0):
                struct.pack_into("<2f", data, base + params, 1.0, 1.0)
                changes.append((aid, old))
    return bytes(data), changes


def check(binder):
    """Opacity events that are still below 1 (should be none after `opaque`)."""
    from er_text import bnd4
    left = []
    for name, tae in bnd4(binder).items():
        if name.lower().endswith(".tae"):
            left += [(aid, struct.unpack_from("<2f", tae, p)) for aid, k, p in _tae_events(tae)
                     if k == OPACITY and p and struct.unpack_from("<2f", tae, p) != (1.0, 1.0)]
    return left


if __name__ == "__main__":
    sys.path.insert(0, str(Path(__file__).parent))
    from dcx_krak import decompress
    raw = decompress(Path(sys.argv[1]).read_bytes())
    new, changes = opaque(raw)
    print(f"{len(changes)} opacity events set to 1 in {len({a for a, _ in changes})} animations: "
          + ", ".join(f"{a} {o[0]:g}->{o[1]:g}" for a, o in changes))
    diff = sum(x != y for x, y in zip(raw, new))
    print(f"{diff} bytes changed of {len(raw)}; still below 1 after: {check(new)}")

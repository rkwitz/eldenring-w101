"""Round 27 (B1): the meshes of an Elden Ring character for a dialogue portrait, posed, with their textures picked,
for tools/blender_npc_portraits.py (which builds and renders them in Blender). numpy + soulstruct only, so it runs in
Blender's own Python (soulstruct comes with the soulstruct-blender add-on).

Elden Ring names textures in its material files (MATBIN, not read here), so a mesh's albedo is found the way the
book's software renderer (hud_orbs/src/model.rs texture_for) does: the textures of the material's "P[NAME]" whose
name holds a word of the material's name, else that part's biggest albedo. Characters with their own model (chr)
use a per-model material -> texture table (w101_npc_portraits.CHRS) where the heuristic fails.

Pose: c0000 people take the player's idle animation (book\\er\\idle_pose.tsv from er_idle_pose.py: per bone the
reference and animated world transforms, row vectors p * M + t); a part's bone follows the body bone of its name,
else its nearest named ancestor, else the nearest body bone; cloth bones hang in toward the bone they hang from
(model.rs Pose::drape). chr models stay in their bind pose (their FLVER rest pose is their standing pose).

Coordinates: game space (x, y up, z; the character faces -z) is returned as Blender's (x, z, y): a mirror, as
soulstruct-blender does, because the game is left-handed; triangle winding is flipped with it.
"""
from __future__ import annotations

import os
from pathlib import Path

import numpy as np

from soulstruct.containers import Binder
from soulstruct.containers.tpf import TPF
from soulstruct.flver import FLVER

# Wizard101 NPC -> Elden Ring stand-in. c0000 people: vanilla CharaInitParam rows (EquipParamProtector equipModelId
# and invisibleFlag_SexVerNN bits of each worn piece, FaceParam hair_partsId -> hr_a_<id>, eyebrow_partsId ->
# fg_a_20<id>, beard_partsId -> fg_a_30<id> (0 = none), hair/skin colours), read with tools/er_params.py.
HIDE_23260 = [6, 7, 8, 9, 19, 20, 21, 41, 42, 43, 45, 46, 47, 49, 50, 52, 54, 55, 57, 59, 66, 68, 69, 70, 73, 77, 11,
              13, 18, 25, 26, 28, 29, 30, 12, 14, 15, 16, 72]
HIDE_23110 = [6, 7, 8, 9, 10, 19, 20, 21, 40, 41, 42, 45, 46, 48, 49, 50, 51, 53, 55, 57, 59, 66, 68, 69, 70, 73, 77,
              81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 11, 13, 18, 24, 26, 28, 29, 30, 12, 14, 15, 16, 72]
HIDE_23510 = [17, 23, 33, 37, 60, 61, 64, 65, 75, 19, 21, 40, 41, 42, 44, 46, 47, 48, 50, 51, 53, 54, 55, 56, 57, 58,
              59, 66, 67, 68, 70, 73, 77, 12, 72]
HIDE_23190 = [0, 1, 2, 3, 4, 17, 32, 34, 37, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 75, 76, 78, 79, 80, 6, 7, 8, 9,
              10, 19, 20, 21, 40, 41, 42, 44, 46, 47, 49, 50, 51, 54, 55, 56, 57, 59, 70, 73, 77, 81, 82, 83, 84, 85,
              86, 87, 88, 89, 90, 91, 92, 93, 94, 11, 13, 18, 25, 26, 28, 29, 30, 12, 14, 15, 16, 72]
HIDE_23130 = [6, 7, 8, 19, 20, 21, 40, 42, 43, 45, 47, 48, 49, 50, 51, 53, 54, 55, 56, 57, 58, 59, 67, 68, 69, 73, 77,
              12, 14, 15, 16, 72]

PEOPLE = {
    # slug: sex, armour parts, hide bits, hair, eyebrow, beard, hair RGB, skin RGB (CharaInitParam row)
    "private_connelly": ("m", ["bd_m_1331", "am_m_1330", "lg_m_1330"], HIDE_23260, 9, 5, 6, (79, 69, 59), (189, 137, 104)),  # Knight Bernahl 23260
    "private_oryan": ("m", ["bd_m_1652", "am_m_1650", "lg_m_1650"], HIDE_23110, 5, 3, 2, (115, 108, 80), (221, 180, 131)),  # Castellan Edgar 23110
    "ceren_nightchant": ("m", ["hd_m_1240", "bd_m_1240", "lg_m_1240"], HIDE_23510, 112, 6, 0, (233, 223, 175), (230, 194, 172)),  # Brother Corhyn 23510
    "diego": ("m", ["hd_m_1220", "bd_m_1220", "am_m_1220", "lg_m_1220"], HIDE_23190, 9, 12, 0, (255, 228, 217), (242, 212, 206)),  # D, Hunter of the Dead 23190
    "olivia_rya": ("f", ["bd_f_1531", "lg_m_1480"], HIDE_23130, 110, 0, 0, (255, 223, 191), (255, 197, 191)),  # Rya the Scout 23130
}
# chr models: slug -> model id
MODELS = {
    "olivia_dawnwillow": "c2170",  # Finger Reader Enia
    "lady_oriel": "c5330",  # St. Trina
    "ugo_kalahad": "c3200",  # Merchant Kale
    "merle_ambrose": "c2050",  # Ranni the Witch
    "lady_blackhope": "c3300",  # Nox Swordstress
    "rattlebones": "c4960",  # Giant Skeleton Torso
}
# Each chr's materials -> its cXXXX_<x>_a texture (a tuple: a flat colour; None: left out); others by heuristic.
CHRS = {
    "c2170": {"Body": "Body", "Fabric": "Fabric", "Fabric_cloth": "Fabric", "Fraying": "Fraying",
              "Fraying_cloth": "Fraying", "Hair": (200, 196, 186), "Metal": "Body", "WP_Wand": "WP_Wand"},
    "c5330": {"Eyebrow": "Eyebrow", "Eyelash": "Eyelash", "Eyelash_Under": "Eyelash", "Fabric": "Trina_Body",
              "Face": "Face", "Hair_glow": None, "Hair_glow_quick": None, "Hair_glow_slow": None,
              "Hair_model": "Hair_model", "Hair_model_back": "Hair_model", "Mouse": "Mouth",
              "Plant_Body": "Plant_Body", "Plant_Body_2": "Plant_Parts", "Plant_Body_Veil": "Petal_color",
              "Trina_Body": "Trina_Body", "stem": "Plant_Parts"},
    "c2050": {"BD": "BD", "BD_Mant": "BD", "BD_Mant_Fur": "BD", "BD_Mant_Trim": "BD", "BD_Skin_Fiber": None,
              "BD_Skin_SSS": "BD_Skin", "BD_cloth": "BD", "FC_Eye_Blue": "FC_Eye", "FC_Eye_Occulusion": None,
              "FC_Face_SSS": "FC_Face", "FC_Ghost": None, "FC_Ghost_Eye": None, "FC_Ghost_Hair": None,
              "FC_Ghost_Hair_Cloth": None, "FC_Ghost_Hair_Tip": None, "FC_Ghost_Hair_Tip_Cloth": None,
              "FC_Ghost_Pattern": None, "FC_Mouth_SSS": "Mouth", "HD": "HD", "HD_Crystal": "HD",
              "HD_Fur_Shell": None, "HD_Metal": "HD", "HR": "HR", "LG": "LG", "LG_cloth": "LG",
              "WP_A_1350": "WP_A_1350", "WP_A_1350_Crystal": "WP_A_1350"},
    "c3300": {"AM_fabric": "AM", "BD_fabric": "BD", "BD_fabric_cloth": "BD", "BD_leather": "BD", "BD_metal": "BD",
              "BD_scale": "BD", "FC_Eye": "eye", "FC_Skin": "face", "HD_fabric": "HD", "HD_fabric_cloth": "HD",
              "HD_metal": "HD", "LG_fabric": "LG", "LG_fabric_cloth": "LG", "LG_leather": "LG", "LG_metal": "LG",
              "LG_scale": "LG", "belt": "BD",
              **{k: None for k in ["FMA_BD_fabric", "FMA_BD_fabric_cloth", "FMA_BD_leather", "FMA_BD_metal",
                                   "FMA_FC_Skin", "FMA_HD_fabric", "FMA_HD_fabric2", "FMA_HD_metal",
                                   "FMB_BD_fabric", "FMB_BD_fabric_cloth", "FMB_HD_fabric", "FMB_HD_metal",
                                   "WP_A_0217", "WP_A_0818", "WP_A_1323"]}},
    "c4960": {"Skeleton": "skeleton", "ring": "skeleton"},
    # Merchant Kale: face and hair have their own sheets; the guitar and stick are left out of the portrait.
    "c3200": {"face_Skin": "FC", "face_Hair": "FC_Hair", "HD_Variation": "HD_Variation", "BD_Pouch": "BD_Pouch",
              "WP_Guitar": None, "WP_Stick": None, "BD_ShellFur": None},
}
# Shared strand sheets (common_body.tpf) for chr hair that has no texture of its own: material -> (sheet, colour).
CHR_HAIR = {"c2170": {"Hair": ("aat500_hair_01_a", (200, 196, 186))}}
# NpcParam modelDispMask bits the game shows (meshes "#NN#..." with other NN are hidden): Kale's 32000000.
DISP = {"c3200": {0, 1, 11, 20, 25, 26, 30}}
# Eye iris colour baked into the eye white (model.rs eye()).
IRIS = (92.0, 64.0, 38.0)


class Files:
    """Game files: `dec` (a folder of already decompressed copies, named without .dcx) first, else the game's."""

    def __init__(self, game: Path, dec: Path | None = None):
        self.game, self.dec = Path(game), Path(dec) if dec else None
        self._tpfs: dict[str, dict[str, bytes]] = {}

    def read(self, sub: str, name: str) -> bytes | None:
        if self.dec and (self.dec / name).is_file():
            return (self.dec / name).read_bytes()
        path = self.game / sub / f"{name}.dcx"
        if not path.is_file():
            return None
        from soulstruct.dcx import decompress
        data, _ = decompress(path)
        return bytes(data)

    def binder(self, sub: str, name: str):
        data = self.read(sub, name)
        return Binder.from_bytes(data) if data else None

    def textures(self, key: str, binders) -> dict[str, bytes]:
        """DDS bytes by lower-case stem, from every TPF in the binders (cached by key)."""
        if key not in self._tpfs:
            out = {}
            for b in binders:
                if b is None:
                    continue
                for e in b.entries:
                    if e.name.lower().endswith(".tpf"):
                        for t in TPF.from_bytes(e.data).textures:
                            out[t.stem.lower()] = bytes(t.data)
            self._tpfs[key] = out
        return self._tpfs[key]


def _common(self):
    """common_body.tpf: the shared hair, beard and skin textures."""
    if "common" not in self._tpfs:
        data = self.read("parts", "common_body.tpf")
        self._tpfs["common"] = {t.stem.lower(): bytes(t.data) for t in TPF.from_bytes(data).textures} if data else {}
    return self._tpfs["common"]


Files.common = _common


def flvers(binder):
    return [FLVER.from_bytes(e.data) for e in binder.entries if e.name.lower().endswith(".flver")]


# ---- pose maths (row vectors: p' = p @ M + t), as model.rs ----
def _rx(a):
    s, c = np.sin(a), np.cos(a)
    return np.array([[1, 0, 0], [0, c, s], [0, -s, c]])


def _ry(a):
    s, c = np.sin(a), np.cos(a)
    return np.array([[c, 0, -s], [0, 1, 0], [s, 0, c]])


def _rz(a):
    s, c = np.sin(a), np.cos(a)
    return np.array([[c, s, 0], [-s, c, 0], [0, 0, 1]])


def bind_pose(bones):
    """Each bone's bind world (M, t): scale, rotation X, Z, Y, translation, then the parent's."""
    out = [None] * len(bones)

    def get(i, depth=0):
        if out[i] is not None:
            return out[i]
        b = bones[i]
        r = np.diag(list(b.scale)) @ _rx(b.rotate[0]) @ _rz(b.rotate[2]) @ _ry(b.rotate[1])
        t = np.array(list(b.translate), float)
        p = b.parent_bone_index
        if 0 <= p < len(bones) and depth < 64:
            pr, pt = get(p, depth + 1)
            r, t = r @ pr, t @ pr + pt
        out[i] = (r, t)
        return out[i]

    return [get(i) for i in range(len(bones))]


def load_idle(path):
    rows = ({}, {})
    for line in Path(path).read_text().splitlines():
        f = line.split("\t")
        if len(f) < 5 or f[0] not in ("R", "A"):
            continue
        m = np.array([float(v) for v in f[3].split()]).reshape(3, 3)
        t = np.array([float(v) for v in f[4].split()])
        rows[f[0] == "A"][f[1]] = (m, t)
    ref, anim = rows
    out = {}
    for name, (am, at) in anim.items():
        if name in ref:
            rm, rt = ref[name]
            r = np.linalg.inv(rm) @ am
            out[name] = (r, at - rt @ r)
    return out


class Pose:
    """The body's bones (c0000 face/body FLVER) moved from bind to the idle pose."""

    def __init__(self, body: FLVER | None, idle: dict | None):
        self.names, self.bind, self.delta = {}, [], []
        if body is None:
            return
        self.bind = bind_pose(body.bones)
        self.names = {b.name: i for i, b in enumerate(body.bones)}
        own = [idle.get(b.name) if idle else None for b in body.bones]
        for i in range(len(body.bones)):
            j, d = i, None
            for _ in range(64):
                if j < 0 or j >= len(body.bones):
                    break
                if own[j] is not None:
                    d = own[j]
                    break
                j = body.bones[j].parent_bone_index
            self.delta.append(d if d is not None else (np.eye(3), np.zeros(3)))

    def head(self):
        """(posed head position, facing) in game space, or None."""
        h = self.names.get("Head")
        if h is None:
            return None
        r, t = self.delta[h]
        return self.bind[h][1] @ r + t

    def skin(self, f: FLVER, pos, nrm, bone_idx, weights):
        if not self.delta:
            return pos, nrm
        own = bind_pose(f.bones)
        names = [b.name for b in f.bones]
        keys = [(n, k) for n, k in self.names.items() if not (n.startswith("Ctrl") or n.startswith("FC_") or "Finger" in n)]
        kpos = np.array([self.bind[k][1] for _, k in keys])
        mapping = []
        for i in range(len(f.bones)):
            j, found = i, None
            for _ in range(64):
                if j < 0 or j >= len(f.bones):
                    break
                if names[j] in self.names:
                    found = self.names[names[j]]
                    break
                j = f.bones[j].parent_bone_index
            if found is None:
                found = keys[int(np.argmin(((kpos - own[i][1]) ** 2).sum(1)))][1]
            mapping.append(found)
        # Cloth bones hang in toward the bone they hang from (model.rs Pose::drape).
        mats = []
        for i, b in enumerate(f.bones):
            r, t = self.delta[mapping[i]]
            if b.name.startswith("[cloth]"):
                j = b.parent_bone_index
                for _ in range(16):
                    if j < 0 or not f.bones[j].name.startswith("[cloth]"):
                        break
                    j = f.bones[j].parent_bone_index
                if j >= 0:
                    r, t = self.delta[mapping[j]]
                    bb, a = own[i][1], own[j][1]
                    drop = np.clip((a[1] - bb[1]) / 0.6, 0, 1)
                    pull = np.array([(bb[0] - a[0]) * 0.75 * drop, 0.0, max(bb[2] - a[2], 0.0) * 0.5 * drop])
                    t = t - pull @ r
            mats.append((r, t))
        R = np.stack([m[0] for m in mats])
        T = np.stack([m[1] for m in mats])
        w = weights / np.maximum(weights.sum(1, keepdims=True), 1e-6)
        idx = np.clip(bone_idx, 0, len(mats) - 1)
        p = np.zeros_like(pos)
        n = np.zeros_like(nrm)
        for k in range(4):
            wk = w[:, k:k + 1]
            p += wk * (np.einsum("vi,vij->vj", pos, R[idx[:, k]]) + T[idx[:, k]])
            n += wk * np.einsum("vi,vij->vj", nrm, R[idx[:, k]])
        none = weights.sum(1) < 1e-6
        p[none], n[none] = pos[none], nrm[none]
        return p, n


# ---- textures ----
def texture_for(material: str, own: str, names) -> str | None:
    """model.rs texture_for: the albedo stem for a material path like 'P[BD_M_1330]_Fabric.matxml'."""
    m = material.lower().replace("\\", "/").split("/")[-1]
    if "p[" in m and "]" in m:
        key, rest = m.split("p[", 1)[1].split("]", 1)
        rest = rest.replace(".matxml", "").strip("_")
    else:
        key, rest = own, m
    for k in (key, own):
        cands = [n for n in names if n.startswith(k) and (n.endswith("_a") or n.endswith("_a_l"))]
        if not cands:
            continue

        def middle(n):
            s = n[len(k):]
            for suf in ("_l", "_a"):
                if s.endswith(suf):
                    s = s[: -len(suf)]
            return s.strip("_")

        for n in cands:
            mid = middle(n)
            if mid and (mid in rest or (rest and rest in mid)):
                return n
        for n in cands:
            if any(len(w) >= 3 and w not in ("color", "colour", "base") and w in rest for w in middle(n).split("_")):
                return n
        return max(cands, key=lambda n: (names[n], -len(n)))
    return None


def dds_size(d: bytes) -> int:
    return int.from_bytes(d[12:16], "little") * int.from_bytes(d[16:20], "little") if d[:4] == b"DDS " else 0


class Mesh:
    """One mesh for Blender: vertices (Blender space), triangles, uvs, normals and its look."""

    def __init__(self, name, pos, nrm, uv, tris, kind, albedo=None, colour=(0.6, 0.6, 0.6), alpha=None,
                 normal=None, uv2=None):
        self.name, self.pos, self.nrm, self.uv, self.tris = name, pos, nrm, uv, tris
        self.kind = kind  # "solid", "hair", "eye", "skin", "metal"
        self.albedo, self.colour, self.alpha, self.normal, self.uv2 = albedo, colour, alpha, normal, uv2


def _arrays(m):
    # Some meshes split their vertex fields over two arrays (positions in one, the rest in the other): merge them,
    # first array wins.
    v = {}
    for a in m.vertex_arrays:
        for n in a.array.dtype.names:
            v.setdefault(n, a.array[n])
    names = tuple(v)
    pos = np.asarray(v["position"], float)
    nrm = np.asarray(v["normal"], float) if "normal" in names else np.zeros_like(pos)
    uv = np.asarray(v["uv_0"], float) if "uv_0" in names else np.zeros((len(pos), 2))
    uv1 = np.asarray(v["uv_1"], float) if "uv_1" in names else None
    bi = np.asarray(v["bone_indices"], int) if "bone_indices" in names else np.zeros((len(pos), 4), int)
    bw = np.asarray(v["bone_weights"], float) if "bone_weights" in names else np.zeros((len(pos), 4))
    if m.bone_indices is not None and len(m.bone_indices):  # local bone lists (older FLVERs)
        bi = np.asarray(m.bone_indices)[np.clip(bi, 0, len(m.bone_indices) - 1)]
    if "bone_weights" not in names or not bw.any():
        # Rigid meshes: all on the default bone.
        bi = np.full((len(pos), 4), m.default_bone_index)
        bw = np.zeros((len(pos), 4))
        bw[:, 0] = 1
        if "normal_w" in names:
            bi[:, 0] = np.asarray(v["normal_w"]).reshape(-1)
    tris = np.asarray(m.triangulate_flver2(), int).reshape(-1, 3)
    return pos, nrm, uv, uv1, bi, bw, tris


def _to_blender(p):
    return p[:, [0, 2, 1]]


def _group(name: str):
    if name.startswith("#"):
        try:
            return int(name[1:].split("#")[0])
        except ValueError:
            return None
    return None


def person(files: Files, slug: str, idle: dict) -> tuple[list[Mesh], dict, np.ndarray]:
    """A c0000 stand-in's meshes, its DDS textures by stem and its head position (Blender space)."""
    sex, armour, hide, hair, brow, beard, hair_rgb, skin_rgb = PEOPLE[slug]
    hide = set(hide)
    stems = [f"fc_{sex}_0000", "fg_a_0100", "fg_a_1500"]
    if hair:
        stems.append(f"hr_a_{hair:04d}")
    if brow:
        stems.append(f"fg_a_20{brow:02d}")
    if beard:
        stems.append(f"fg_a_30{beard:02d}")
    stems += armour
    binders = {}
    for s in stems:
        b = files.binder("parts", f"{s}_l.partsbnd") or files.binder("parts", f"{s}.partsbnd")
        if b is None:
            print(f"portrait {slug}: {s} not found, left out")
            continue
        binders[s] = b
    tex = dict(files.common())
    for s, b in binders.items():
        tex.update(files.textures(s, [b]))
    sizes = {n: dds_size(d) for n, d in tex.items()}
    body = flvers(binders[stems[0]])[0]
    pose = Pose(body, idle)
    hair_c = tuple(c / 255 for c in hair_rgb)
    skin_c = tuple(c / 255 for c in skin_rgb)
    meshes = []
    for s, b in binders.items():
        is_face = s.startswith("fg_")
        is_hair = s.startswith("hr_") or s.startswith("fg_a_2") or s.startswith("fg_a_3")
        is_eye = s == "fg_a_1500"
        for f in flvers(b):
            for i, m in enumerate(f.meshes):
                g = _group(m.material.name)
                if g is not None and g in hide:
                    continue
                mat = m.material.mat_def_path.lower()
                lname = m.material.name.lower()
                if (is_face and "fg_a" not in mat) or "eyeao" in mat or "shadow" in mat or "_hairy" in lname:
                    continue
                if s.startswith("fc_") and "hair" in mat:
                    continue
                pos, nrm, uv, uv1, bi, bw, tris = _arrays(m)
                if not len(tris):
                    continue
                pos, nrm = pose.skin(f, pos, nrm, bi, bw)
                name = f"{s}:{i}:{m.material.name}"
                if is_hair:
                    own_n = next((n for n in tex if n.startswith(s) and n.endswith("_n_l")), None)
                    # Strand cards take the shared strand masks; a part's base shell its own _n alpha.
                    alpha = own_n if (own_n and not s.startswith("hr_")) else "aat500_pchair_n"
                    if "beard_hair" in mat or ("beard" in mat and not own_n):
                        alpha = "aat500_pcbeard_n"
                    meshes.append(Mesh(name, pos, nrm, uv, tris, "hair", colour=hair_c, alpha=alpha))
                elif is_eye:
                    white = next((n for n in tex if n.startswith("fg_a_1500_eye_a")), None)
                    meshes.append(Mesh(name, pos, nrm, uv, tris, "eye", albedo=white, alpha="fg_a_1500_eye_3m_l"))
                else:
                    a = texture_for(mat, s, sizes)
                    kind = "skin" if s.startswith(("fc_", "fg_")) else ("metal" if "metal" in mat or "chain" in mat else "solid")
                    normal = None
                    if a:
                        cand = a.replace("_a_l", "_n_l") if a.endswith("_a_l") else a[:-2] + "_n"
                        normal = cand if cand in tex else None
                    meshes.append(Mesh(name, pos, nrm, uv, tris, kind, albedo=a, colour=skin_c, normal=normal))
    for m in meshes:
        m.pos, m.nrm = _to_blender(m.pos), _to_blender(m.nrm)
        m.tris = m.tris[:, ::-1]
    head = pose.head()
    return meshes, tex, _to_blender(head[None])[0] if head is not None else None


def model(files: Files, slug: str) -> tuple[list[Mesh], dict, np.ndarray]:
    cid = MODELS[slug]
    chrbnd = files.binder("chr", f"{cid}.chrbnd")
    texbnd = files.binder("chr", f"{cid}_h.texbnd")
    tex = dict(files.textures(cid, [texbnd, chrbnd]))
    tex.update(files.common())
    sizes = {n: dds_size(d) for n, d in tex.items()}
    table = CHRS.get(cid, {})
    f = flvers(chrbnd)[0]
    bind = bind_pose(f.bones)
    meshes = []
    for i, m in enumerate(f.meshes):
        mat = m.material.mat_def_path
        leaf = mat.replace("\\", "/").split("/")[-1].replace(".matxml", "")
        short = leaf.split("]", 1)[1].lstrip("_") if "]" in leaf else leaf
        lname = short.lower()
        if "shadow" in lname or "ghost" in lname.split("_")[:2] or lname.startswith("fma_") or lname.startswith("fmb_"):
            if short not in table or table.get(short) is None:
                continue
        g = _group(m.material.name)
        if cid in DISP and g is not None and g not in DISP[cid]:
            continue
        pick = table.get(short, "?")
        if pick is None:
            continue
        if short in CHR_HAIR.get(cid, {}):
            sheet, rgb = CHR_HAIR[cid][short]
            pos, nrm, uv, uv1, bi, bw, tris = _arrays(m)
            meshes.append(Mesh(f"{cid}:{i}:{short}", pos, nrm, uv, tris, "hair",
                               colour=tuple(c / 255 for c in rgb), alpha=sheet))
            continue
        pos, nrm, uv, uv1, bi, bw, tris = _arrays(m)
        if not len(tris):
            continue
        # Skinned meshes are stored in bind pose already (model space).
        colour = (0.6, 0.6, 0.6)
        if isinstance(pick, tuple):
            albedo, colour = None, tuple(c / 255 for c in pick)
        elif pick != "?":
            albedo = next((n for n in (f"{cid}_{pick}_a".lower(), f"{cid}_{pick}_a_l".lower()) if n in tex), None)
        else:
            albedo = texture_for(f"P[{cid}]_{short}", cid, sizes)
        normal = None
        if albedo:
            cand = albedo[:-2] + "_n"
            normal = cand if cand in tex else None
        kind = "hair" if "hair" in lname else ("skin" if any(w in lname for w in ("skin", "face", "fc_")) else
                                                ("metal" if "metal" in lname else "solid"))
        alpha = None
        if kind == "hair" and albedo is None and not isinstance(pick, tuple):
            kind = "solid"
        meshes.append(Mesh(f"{cid}:{i}:{short}", pos, nrm, uv, tris, kind if kind != "hair" else "solid_alpha",
                           albedo=albedo, colour=colour, alpha=alpha, normal=normal))
    for m in meshes:
        m.pos, m.nrm = _to_blender(m.pos), _to_blender(m.nrm)
        m.tris = m.tris[:, ::-1]
    names = {b.name: k for k, b in enumerate(f.bones)}
    head = None
    for n in ("Head", "head", "Body_Head", "Neck", "FC_Head"):
        if n in names:
            head = _to_blender(bind[names[n]][1][None])[0]
            break
    return meshes, tex, head


def build(files: Files, slug: str, idle: dict):
    return person(files, slug, idle) if slug in PEOPLE else model(files, slug)


def eye_pixels(white: np.ndarray, mask: np.ndarray) -> np.ndarray:
    """model.rs eye(): ER's eye albedo is only the white; the iris and pupil come from the _3m mask (green = iris and
    pupil, blue = not pupil, red = iris fibres). Both RGBA float arrays (h, w, 4), same size."""
    g, b, r = mask[..., 1], mask[..., 2], mask[..., 0]
    iris, pupil = g * b, g * (1 - b)
    out = white.copy()
    for k in range(3):
        ic = IRIS[k] / 255.0 * (0.75 + 0.6 * r)
        out[..., k] = white[..., k] * (1 - iris - pupil) + ic * iris + (8 / 255.0) * pupil
    out[..., 3] = 1
    return np.clip(out, 0, 1)


if __name__ == "__main__":  # quick check without Blender
    import sys
    fs = Files(Path(sys.argv[1]), Path(sys.argv[2]) if len(sys.argv) > 2 else None)
    idle = load_idle(sys.argv[3]) if len(sys.argv) > 3 else {}
    for s in list(PEOPLE) + list(MODELS):
        ms, tx, hd = build(fs, s, idle)
        print(s, len(ms), "meshes", None if hd is None else hd.round(3),
              sorted({(m.kind, m.albedo) for m in ms}, key=str)[:12])
    os.sys.exit(0)

"""Read and write Havok 2018 tagfiles (TAG0), as Elden Ring stores collision (stdlib only).

A tagfile is a DATA blob of objects, a TYPE section describing every type in it, and an INDX section: ITEM (each
object or array in DATA, with its type, offset and count) and PTCH (where DATA holds item indices, grouped by the type
of the field). `read` turns DATA into Python values, `write` lays them out again reusing the file's own TYPE section, so
any object graph made of the file's types can be written back.

Values: a class is a `Record` (dict of member values, with `.type`), a pointer is a `Record` or None, an hkArray is a
list (with `.type` for its element type via `Array`), a string is str, tuples are lists, ints and floats are numbers.
"""

import struct

KIND_MASK = 0x0F
VOID, OPAQUE, BOOL, STRING, INT, FLOAT, POINTER, CLASS, ARRAY = range(9)
TUPLE_FLAG = 0x20
SIGNED = 0x200
INT_SIZES = {0x2000: 1, 0x4000: 2, 0x8000: 4, 0x10000: 8}
FLOAT_FORMATS = {4: "<f", 8: "<d"}


def _packed(data, pos):
    b = data[pos]
    if b < 0x80:
        return b, pos + 1
    if b < 0xC0:
        return ((b << 8) | data[pos + 1]) & 0x3FFF, pos + 2
    if b < 0xE0:
        return int.from_bytes(data[pos:pos + 3], "big") & 0x1FFFFF, pos + 3
    if b < 0xE8:
        return int.from_bytes(data[pos:pos + 4], "big") & 0x7FFFFFF, pos + 4
    if b == 0xE8:
        return int.from_bytes(data[pos + 1:pos + 5], "big"), pos + 5
    if b == 0xF0:
        return int.from_bytes(data[pos + 1:pos + 9], "big"), pos + 9
    raise ValueError(f"bad packed int {b:#x} at {pos:#x}")


def _sections(data, start, end):
    out = []
    while start < end:
        size = int.from_bytes(data[start:start + 4], "big") & 0x3FFFFFFF
        out.append((data[start + 4:start + 8].decode(), start, size))
        start += size
    return out


class Type:
    def __init__(self, index):
        self.index = index
        self.name, self.templates, self.parent = None, [], None
        self.flags = 0
        self.format = self.pointer = self.version = self.size = self.align = self.abstract = None
        self.members, self.interfaces = [], []
        self.hash = None

    def __repr__(self):
        args = ", ".join(f"{n}={v}" for n, v in self.templates)
        return f"<{self.index} {self.name}{'<' + args + '>' if args else ''}>"

    def get(self, attr):
        """Attribute from this type or the nearest parent that sets it."""
        t = self
        while t is not None:
            if getattr(t, attr) is not None:
                return getattr(t, attr)
            t = t.parent
        return None

    def all_members(self):
        chain = []
        t = self
        while t is not None:
            chain.append(t)
            t = t.parent
        return [m for t in reversed(chain) for m in t.members]

    @property
    def kind(self):
        return (self.get("format") or 0) & KIND_MASK


class Member:
    def __init__(self, name, flags, offset, type_):
        self.name, self.flags, self.offset, self.type = name, flags, offset, type_

    def __repr__(self):
        return f"{self.name}@{self.offset:#x}:{self.type}"


class Record(dict):
    def __init__(self, type_, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.type = type_

    def __repr__(self):
        return f"{self.type.name}{dict.__repr__(self)}"


class Array(list):
    def __init__(self, type_, *args):
        super().__init__(*args)
        self.type = type_  # element type


class TagFile:
    def __init__(self, data):
        self.raw = data
        top = _sections(data, 0, len(data))
        assert top[0][0] == "TAG0"
        sub = {name: (start, size) for name, start, size in _sections(data, 8, len(data))}
        self.sdk = data[sub["SDKV"][0] + 8:sub["SDKV"][0] + 16].decode()
        self.data_start = sub["DATA"][0] + 8
        self.data = data[self.data_start:sub["DATA"][0] + sub["DATA"][1]]
        type_start, type_size = sub["TYPE"]
        self.type_section = data[type_start:type_start + type_size]
        tsub = {n: (s, z) for n, s, z in _sections(data, type_start + 8, type_start + type_size)}
        isub = {n: (s, z) for n, s, z in _sections(data, sub["INDX"][0] + 8, sub["INDX"][0] + sub["INDX"][1])}
        self._read_types(tsub)
        self._read_index(isub)

    # --- types
    def _strings(self, section):
        start, size = section
        return self.raw[start + 8:start + size].split(b"\0")

    def _read_types(self, tsub):
        tstr = [s.decode() for s in self._strings(tsub["TSTR"])]
        fstr = [s.decode() for s in self._strings(tsub["FSTR"])]
        d = self.raw
        start, size = tsub["TNA1"]
        pos, end = start + 8, start + size
        count, pos = _packed(d, pos)
        self.types = [None] + [Type(i) for i in range(1, count)]
        for t in self.types[1:]:
            name, pos = _packed(d, pos)
            t.name = tstr[name]
            n, pos = _packed(d, pos)
            for _ in range(n):
                tname, pos = _packed(d, pos)
                value, pos = _packed(d, pos)
                tname = tstr[tname]
                t.templates.append((tname, self.types[value] if tname.startswith("t") else value))
        start, size = tsub["TBDY"]
        pos, end = start + 8, start + size
        while pos < end:
            index, pos = _packed(d, pos)
            if index == 0:
                continue
            t = self.types[index]
            parent, pos = _packed(d, pos)
            t.parent = self.types[parent]
            t.flags, pos = _packed(d, pos)
            if t.flags & 0x01:
                t.format, pos = _packed(d, pos)
            if t.flags & 0x02:
                v, pos = _packed(d, pos)
                t.pointer = self.types[v]
            if t.flags & 0x04:
                t.version, pos = _packed(d, pos)
            if t.flags & 0x08:
                t.size, pos = _packed(d, pos)
                t.align, pos = _packed(d, pos)
            if t.flags & 0x10:
                t.abstract, pos = _packed(d, pos)
            if t.flags & 0x20:
                n, pos = _packed(d, pos)
                t.member_count_word = n
                for _ in range(n & 0xFFFF):
                    name, pos = _packed(d, pos)
                    flags, pos = _packed(d, pos)
                    offset, pos = _packed(d, pos)
                    mtype, pos = _packed(d, pos)
                    t.members.append(Member(fstr[name], flags, offset, self.types[mtype]))
            if t.flags & 0x40:
                n, pos = _packed(d, pos)
                for _ in range(n):
                    it, pos = _packed(d, pos)
                    value, pos = _packed(d, pos)
                    t.interfaces.append((self.types[it], value))
            if t.flags & 0x80:
                _, pos = _packed(d, pos)
        if "THSH" in tsub:
            start, size = tsub["THSH"]
            pos = start + 8
            n, pos = _packed(d, pos)
            for _ in range(n):
                index, pos = _packed(d, pos)
                self.types[index].hash = struct.unpack_from("<I", d, pos)[0]
                pos += 4
        self.by_name = {}
        for t in self.types[1:]:
            self.by_name.setdefault(t.name, t)

    def _read_index(self, isub):
        d = self.raw
        start, size = isub["ITEM"]
        self.items = []
        for pos in range(start + 8, start + size, 12):
            flags, offset, count = struct.unpack_from("<III", d, pos)
            self.items.append((self.types[flags & 0xFFFFFF] if flags & 0xFFFFFF else None, flags >> 24, offset, count))
        start, size = isub["PTCH"]
        pos, end = start + 8, start + size
        self.patches = []
        while pos < end:
            index, count = struct.unpack_from("<II", d, pos)
            offsets = struct.unpack_from(f"<{count}I", d, pos + 8)
            self.patches.append((self.types[index], list(offsets)))
            pos += 8 + 4 * count

    # --- values
    def value(self, t, offset):
        kind = t.kind
        fmt = t.get("format") or 0
        d = self.data
        if fmt & TUPLE_FLAG:
            element = t.get("pointer")
            return [self.value(element, offset + i * element.get("size")) for i in range(fmt >> 8)]
        if kind == CLASS:
            rec = Record(t)
            for m in t.all_members():
                rec[m.name] = self.value(m.type, offset + m.offset)
            return rec
        if kind == INT:
            size = next(s for bit, s in INT_SIZES.items() if fmt & bit)
            return int.from_bytes(d[offset:offset + size], "little", signed=bool(fmt & SIGNED))
        if kind == FLOAT:
            if t.get("size") == 2:  # hkHalf16: the top half of a float
                return struct.unpack("<f", b"\0\0" + d[offset:offset + 2])[0]
            return struct.unpack_from(FLOAT_FORMATS[t.get("size")], d, offset)[0]
        if kind == BOOL:
            return d[offset] != 0
        if kind == POINTER:
            index = struct.unpack_from("<Q", d, offset)[0]
            return self.item(index)
        if kind == ARRAY:
            index = struct.unpack_from("<Q", d, offset)[0]
            items = self.item(index, array=True)
            return items if items is not None else Array(t.get("pointer"))
        if kind == STRING:
            index = struct.unpack_from("<Q", d, offset)[0]
            chars = self.item(index, array=True)
            return None if chars is None else bytes(chars).rstrip(b"\0").decode()
        if kind in (VOID, OPAQUE):
            return d[offset:offset + (t.get("size") or 0)]
        raise ValueError(f"unknown kind {kind} for {t}")

    def item(self, index, array=False):
        if index == 0:
            return None
        if index in self._cache:
            return self._cache[index]
        t, flags, offset, count = self.items[index]
        if array or flags & 0x20:
            out = Array(t)
            self._cache[index] = out
            size = t.get("size")
            out.extend(self.value(t, offset + i * size) for i in range(count))
        else:
            out = Record(t)
            self._cache[index] = out
            out.update(self.value(t, offset))
        return out

    def root(self):
        self._cache = {}
        return self.item(1)


def read(data):
    tag = TagFile(data)
    return tag, tag.root()


# --- writing
#
# Layout follows Havok's own writer, so an unchanged file comes back byte for byte: objects (pointer targets) are laid
# out one at a time in index order. Each object's fields are scanned in order: a pointer gets the next item index (its
# object comes later), an array gets the next index and its storage is reserved straight away, and strings get indices
# after the rest of that block. Array contents are scanned breadth first after their owner.

class _Writer:
    def __init__(self, tag):
        self.tag = tag
        self.data = bytearray()
        self.items = [(0, 0, 0)]
        self.patches = {}
        self.objects = []  # (index, record) waiting to be laid out
        self.indices = {}

    def reserve(self, size, align):
        while len(self.data) % align:
            self.data.append(0)
        offset = len(self.data)
        self.data += bytes(size)
        return offset

    def new_item(self, key):
        index = len(self.items)
        self.items.append(None)
        if key is not None:
            self.indices[key] = index
        return index

    def patch(self, field_type, offset, index):
        struct.pack_into("<Q", self.data, offset, index)
        self.patches.setdefault(field_type.index, []).append(offset)

    def run(self, root):
        self.objects.append((self.new_item(id(root)), root))
        while self.objects:
            index, obj = self.objects.pop(0)
            t = obj.type
            offset = self.reserve(t.get("size"), t.get("align") or 1)
            self.items[index] = (t.index | 0x10 << 24, offset, 1)
            blocks = [(t, offset, [obj])]
            while blocks:
                element, start, values = blocks.pop(0)
                strings = []
                for i, v in enumerate(values):
                    self.put(element, start + i * element.get("size"), v, blocks, strings)
                for field_type, at, text in strings:
                    chars = (text + "\0").encode()
                    item = self.new_item(None)
                    char = self.tag.by_name["char"]
                    pos = self.reserve(len(chars), 2)
                    self.data[pos:pos + len(chars)] = chars
                    self.items[item] = (char.index | 0x20 << 24, pos, len(chars))
                    self.patch(field_type, at, item)

    def put(self, t, offset, v, blocks, strings):
        kind = t.kind
        fmt = t.get("format") or 0
        if fmt & TUPLE_FLAG:
            element = t.get("pointer")
            for i, x in enumerate(v):
                self.put(element, offset + i * element.get("size"), x, blocks, strings)
        elif kind == CLASS:
            for m in t.all_members():
                self.put(m.type, offset + m.offset, v[m.name], blocks, strings)
        elif kind == INT:
            size = next(s for bit, s in INT_SIZES.items() if fmt & bit)
            self.data[offset:offset + size] = int(v).to_bytes(size, "little", signed=bool(fmt & SIGNED))
        elif kind == FLOAT:
            if t.get("size") == 2:
                self.data[offset:offset + 2] = struct.pack("<f", v)[2:]
            else:
                struct.pack_into(FLOAT_FORMATS[t.get("size")], self.data, offset, v)
        elif kind == BOOL:
            self.data[offset] = 1 if v else 0
        elif kind == POINTER:
            if v is not None:
                index = self.indices.get(id(v))
                if index is None:
                    index = self.new_item(id(v))
                    self.objects.append((index, v))
                self.patch(t, offset, index)
        elif kind == ARRAY:
            count = len(v) if v else 0
            if count:
                element = v.type
                index = self.new_item(id(v))
                start = self.reserve(element.get("size") * count, 16)
                self.items[index] = (element.index | 0x20 << 24, start, count)
                blocks.append((element, start, v))
                self.patch(t, offset, index)
        elif kind == STRING:
            if v is not None:
                strings.append((t, offset, v))
        elif kind in (VOID, OPAQUE):
            self.data[offset:offset + len(v)] = v


def _section(name, body, flags=0x40):
    return (len(body) + 8 | flags << 24).to_bytes(4, "big") + name.encode() + body


def write(tag, root):
    """Bytes of a tagfile holding `root`, with `tag`'s TYPE section."""
    w = _Writer(tag)
    w.run(root)
    w.reserve(0, 16)
    item = b"".join(struct.pack("<III", *i) for i in w.items)
    patch = b"".join(struct.pack("<II", t, len(o)) + struct.pack(f"<{len(o)}I", *sorted(o))
                     for t, o in sorted(w.patches.items()))
    body = (_section("SDKV", tag.sdk.encode()) + _section("DATA", bytes(w.data)) + tag.type_section +
            _section("INDX", _section("ITEM", item) + _section("PTCH", patch), 0))
    return _section("TAG0", body, 0)

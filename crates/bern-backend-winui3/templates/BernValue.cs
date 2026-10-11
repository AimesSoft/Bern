// Bern runtime for WinUI 3: schema values and their binary layout.
// Generated build input; do not edit.
//
// Values mirror `bern_protocol::Value`. The layout is the compact one of
// `bern_protocol::codec`: bool = u8, int = i64 LE, float = f64 LE, text =
// u32 LE length + UTF-8, enum = u32 variant + payload, optional = u8 flag +
// value, record = fields in order, list = u32 count + items.

using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Text;

namespace Bern;

/// A schema value. Equality is structural.
public abstract class Value : IEquatable<Value>
{
    public static readonly Value Unit = new UnitValue();
    public static readonly Value None = new OptionalValue(null);

    public static Value Bool(bool value) => new BoolValue(value);
    public static Value Int(long value) => new IntValue(value);
    public static Value Float(double value) => new FloatValue(value);
    public static Value Text(string value) => new TextValue(value);
    public static Value Enum(uint variant, params Value[] fields) => new EnumValue(variant, fields);
    public static Value Some(Value value) => new OptionalValue(value);
    public static Value Record(params Value[] fields) => new RecordValue(fields);
    public static Value List(params Value[] items) => new ListValue(items);
    public static Value List(IEnumerable<Value> items) => new ListValue(items.ToArray());

    public abstract bool Equals(Value? other);
    public override bool Equals(object? obj) => obj is Value value && Equals(value);
    public abstract override int GetHashCode();
    public static bool operator ==(Value? a, Value? b) => a is null ? b is null : a.Equals(b);
    public static bool operator !=(Value? a, Value? b) => !(a == b);
}

public sealed class UnitValue : Value
{
    public override bool Equals(Value? other) => other is UnitValue;
    public override int GetHashCode() => 0;
}

public sealed class BoolValue(bool value) : Value
{
    public bool V { get; } = value;
    public override bool Equals(Value? other) => other is BoolValue b && b.V == V;
    public override int GetHashCode() => V.GetHashCode();
}

public sealed class IntValue(long value) : Value
{
    public long V { get; } = value;
    public override bool Equals(Value? other) => other is IntValue i && i.V == V;
    public override int GetHashCode() => V.GetHashCode();
}

public sealed class FloatValue(double value) : Value
{
    public double V { get; } = value;
    public override bool Equals(Value? other) => other is FloatValue f && f.V.Equals(V);
    public override int GetHashCode() => V.GetHashCode();
}

public sealed class TextValue(string value) : Value
{
    public string V { get; } = value;
    public override bool Equals(Value? other) => other is TextValue t && t.V == V;
    public override int GetHashCode() => V.GetHashCode();
}

public sealed class EnumValue(uint variant, Value[] fields) : Value
{
    public uint Variant { get; } = variant;
    public Value[] Fields { get; } = fields;
    public override bool Equals(Value? other) =>
        other is EnumValue e && e.Variant == Variant && e.Fields.SequenceEqual(Fields);
    public override int GetHashCode() => HashCode.Combine(Variant, Fields.Length);
}

public sealed class OptionalValue(Value? inner) : Value
{
    public Value? Inner { get; } = inner;
    public override bool Equals(Value? other) =>
        other is OptionalValue o && (o.Inner is null ? Inner is null : o.Inner.Equals(Inner));
    public override int GetHashCode() => Inner?.GetHashCode() ?? 0;
}

public sealed class RecordValue(Value[] fields) : Value
{
    public Value[] Fields { get; } = fields;
    public override bool Equals(Value? other) => other is RecordValue r && r.Fields.SequenceEqual(Fields);
    public override int GetHashCode() => Fields.Length;
}

public sealed class ListValue(Value[] items) : Value
{
    public Value[] Items { get; } = items;
    public override bool Equals(Value? other) => other is ListValue l && l.Items.SequenceEqual(Items);
    public override int GetHashCode() => Items.Length;
}

/// A schema type.
public abstract record BType
{
    public static readonly BType Unit = new UnitType();
    public static readonly BType Bool = new BoolType();
    public static readonly BType Int = new IntType();
    public static readonly BType Float = new FloatType();
    public static readonly BType Text = new TextType();
    public static BType Enum(string name) => new EnumType(name);
    public static BType Record(string name) => new RecordType(name);
    public static BType Optional(BType inner) => new OptionalType(inner);
    public static BType List(BType inner) => new ListType(inner);
}

public sealed record UnitType : BType;
public sealed record BoolType : BType;
public sealed record IntType : BType;
public sealed record FloatType : BType;
public sealed record TextType : BType;
public sealed record EnumType(string Name) : BType;
public sealed record RecordType(string Name) : BType;
public sealed record OptionalType(BType Inner) : BType;
public sealed record ListType(BType Inner) : BType;

/// An enum variant: its name and payload field types.
public sealed record VariantDef(string Name, BType[] Fields);

/// Enum and record definitions by name.
public sealed class TypeTable
{
    public Dictionary<string, VariantDef[]> Enums { get; } = new();
    public Dictionary<string, BType[]> Records { get; } = new();

    public TypeTable Enum(string name, params VariantDef[] variants)
    {
        Enums[name] = variants;
        return this;
    }

    public TypeTable Record(string name, params BType[] fields)
    {
        Records[name] = fields;
        return this;
    }
}

public sealed class DecodeException(string message) : Exception(message);

public sealed class Writer
{
    readonly List<byte> bytes = new();

    public byte[] ToArray() => bytes.ToArray();
    public void U8(byte value) => bytes.Add(value);
    public void U32(uint value) => bytes.AddRange(BitConverter.GetBytes(value));
    public void U64(ulong value) => bytes.AddRange(BitConverter.GetBytes(value));
    public void I64(long value) => bytes.AddRange(BitConverter.GetBytes(value));
    public void F64(double value) => bytes.AddRange(BitConverter.GetBytes(value));

    public void Text(string value)
    {
        var encoded = Encoding.UTF8.GetBytes(value);
        U32((uint)encoded.Length);
        bytes.AddRange(encoded);
    }
}

public sealed class Reader(byte[] bytes)
{
    int position;

    public bool IsAtEnd => position == bytes.Length;

    ReadOnlySpan<byte> Take(int count)
    {
        if (position + count > bytes.Length) throw new DecodeException("unexpected end of value");
        var span = new ReadOnlySpan<byte>(bytes, position, count);
        position += count;
        return span;
    }

    public byte U8() => Take(1)[0];
    public uint U32() => BitConverter.ToUInt32(Take(4));
    public ulong U64() => BitConverter.ToUInt64(Take(8));
    public long I64() => BitConverter.ToInt64(Take(8));
    public double F64() => BitConverter.ToDouble(Take(8));

    public string Text()
    {
        var length = (int)U32();
        return Encoding.UTF8.GetString(Take(length));
    }

    /// The bytes after the current position.
    public byte[] Rest() => bytes[position..];
}

public static class Codec
{
    public static void Encode(Value value, BType type, TypeTable types, Writer w)
    {
        switch (value, type)
        {
            case (UnitValue, _): break;
            case (BoolValue b, _): w.U8(b.V ? (byte)1 : (byte)0); break;
            case (IntValue i, _): w.I64(i.V); break;
            case (FloatValue f, _): w.F64(f.V); break;
            case (TextValue t, _): w.Text(t.V); break;
            case (EnumValue e, EnumType et):
                w.U32(e.Variant);
                var variant = types.Enums[et.Name][e.Variant];
                for (var i = 0; i < variant.Fields.Length && i < e.Fields.Length; i++)
                    Encode(e.Fields[i], variant.Fields[i], types, w);
                break;
            case (OptionalValue o, OptionalType ot):
                if (o.Inner is null) w.U8(0);
                else { w.U8(1); Encode(o.Inner, ot.Inner, types, w); }
                break;
            case (RecordValue r, RecordType rt):
                var fields = types.Records[rt.Name];
                for (var i = 0; i < fields.Length && i < r.Fields.Length; i++)
                    Encode(r.Fields[i], fields[i], types, w);
                break;
            case (ListValue l, ListType lt):
                w.U32((uint)l.Items.Length);
                foreach (var item in l.Items) Encode(item, lt.Inner, types, w);
                break;
            default:
                throw new ArgumentException($"cannot encode {value.GetType().Name} as {type}");
        }
    }

    public static Value Decode(BType type, TypeTable types, Reader r)
    {
        switch (type)
        {
            case UnitType: return Value.Unit;
            case BoolType:
                return r.U8() switch
                {
                    0 => Value.Bool(false),
                    1 => Value.Bool(true),
                    var other => throw new DecodeException($"invalid bool byte {other}"),
                };
            case IntType: return Value.Int(r.I64());
            case FloatType: return Value.Float(r.F64());
            case TextType: return Value.Text(r.Text());
            case EnumType et:
                {
                    if (!types.Enums.TryGetValue(et.Name, out var variants))
                        throw new DecodeException($"unknown enum `{et.Name}`");
                    var index = r.U32();
                    if (index >= variants.Length) throw new DecodeException($"`{et.Name}` has no variant #{index}");
                    var fields = variants[index].Fields.Select(field => Decode(field, types, r)).ToArray();
                    return Value.Enum(index, fields);
                }
            case OptionalType ot:
                return r.U8() switch
                {
                    0 => Value.None,
                    1 => Value.Some(Decode(ot.Inner, types, r)),
                    var other => throw new DecodeException($"invalid optional flag {other}"),
                };
            case RecordType rt:
                {
                    if (!types.Records.TryGetValue(rt.Name, out var fields))
                        throw new DecodeException($"unknown record `{rt.Name}`");
                    return Value.Record(fields.Select(field => Decode(field, types, r)).ToArray());
                }
            case ListType lt:
                {
                    var count = (int)r.U32();
                    var items = new Value[count];
                    for (var i = 0; i < count; i++) items[i] = Decode(lt.Inner, types, r);
                    return Value.List(items);
                }
            default:
                throw new DecodeException($"unknown type {type}");
        }
    }

    public static byte[] Bytes(Value value, BType type, TypeTable types)
    {
        var w = new Writer();
        Encode(value, type, types, w);
        return w.ToArray();
    }
}

/// Operations generated expressions use on schema values. The compiler
/// type-checked every expression, so each helper sees the kinds its
/// operator allows; anything else yields a neutral value.
public static class R
{
    public static Value At(Value value, params int[] path)
    {
        foreach (var index in path)
        {
            if (value is RecordValue r && index < r.Fields.Length) value = r.Fields[index];
            else return Value.Unit;
        }
        return value;
    }

    public static bool Truthy(Value value) => value is BoolValue { V: true };
    public static Value Not(Value value) => Value.Bool(!Truthy(value));

    public static Value Neg(Value value) => value switch
    {
        IntValue i => Value.Int(-i.V),
        FloatValue f => Value.Float(-f.V),
        _ => value,
    };

    public static Value Eq(Value a, Value b) => Value.Bool(a.Equals(b));
    public static Value Ne(Value a, Value b) => Value.Bool(!a.Equals(b));

    static int? Order(Value a, Value b) => (a, b) switch
    {
        (IntValue x, IntValue y) => x.V.CompareTo(y.V),
        (FloatValue x, FloatValue y) => x.V.CompareTo(y.V),
        (IntValue x, FloatValue y) => ((double)x.V).CompareTo(y.V),
        (FloatValue x, IntValue y) => x.V.CompareTo(y.V),
        (TextValue x, TextValue y) => string.CompareOrdinal(x.V, y.V),
        _ => null,
    };

    public static Value Lt(Value a, Value b) => Value.Bool(Order(a, b) < 0);
    public static Value Le(Value a, Value b) => Value.Bool(Order(a, b) <= 0);
    public static Value Gt(Value a, Value b) => Value.Bool(Order(a, b) > 0);
    public static Value Ge(Value a, Value b) => Value.Bool(Order(a, b) >= 0);
    public static Value And(Value a, Func<Value> b) => Value.Bool(Truthy(a) && Truthy(b()));
    public static Value Or(Value a, Func<Value> b) => Value.Bool(Truthy(a) || Truthy(b()));
    public static Value Variant(uint index, params Value[] fields) => Value.Enum(index, fields);

    /// The inner value of `Some`, or null for `None`.
    public static Value? Some(Value value) => value is OptionalValue o ? o.Inner : null;

    /// Bern's basic, locale-independent number text, the same on every target.
    public static string Number(double value)
    {
        if (value == Math.Floor(value) && Math.Abs(value) < 1e15)
            return ((long)value).ToString(CultureInfo.InvariantCulture);
        return value.ToString("R", CultureInfo.InvariantCulture);
    }

    public static string Plain(Value value) => value switch
    {
        TextValue t => t.V,
        IntValue i => i.V.ToString(CultureInfo.InvariantCulture),
        FloatValue f => Number(f.V),
        BoolValue b => b.V ? "true" : "false",
        _ => "",
    };

    public static double AsDouble(Value value) => value switch
    {
        FloatValue f => f.V,
        IntValue i => i.V,
        _ => 0,
    };

    public static string AsText(Value value) => value is TextValue t ? t.V : "";
    public static Value[] AsList(Value value) => value is ListValue l ? l.Items : Array.Empty<Value>();
    public static uint? Tag(Value value) => value is EnumValue e ? e.Variant : null;

    public static Value Payload(Value value, int field) =>
        value is EnumValue e && field < e.Fields.Length ? e.Fields[field] : Value.Unit;

    public static string KeyText(Value value) => value switch
    {
        IntValue i => i.V.ToString(CultureInfo.InvariantCulture),
        TextValue t => t.V,
        _ => value.ToString() ?? "",
    };
}

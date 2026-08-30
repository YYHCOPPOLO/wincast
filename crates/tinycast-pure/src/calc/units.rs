//! Measurement units as affine maps onto a category base: `base = value * factor + offset`.

use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UnitCategory {
    Length,
    Weight,
    Temperature,
    Time,
    Area,
    Volume,
    DigitalStorage,
    Angle,
    Speed,
    Pressure,
    DataRate,
}

impl UnitCategory {
    pub(crate) fn display_name(self) -> &'static str {
        match self {
            Self::Length => "Length",
            Self::Weight => "Weight",
            Self::Temperature => "Temperature",
            Self::Time => "Time",
            Self::Area => "Area",
            Self::Volume => "Volume",
            Self::DigitalStorage => "Digital Storage",
            Self::Angle => "Angle",
            Self::Speed => "Speed",
            Self::Pressure => "Pressure",
            Self::DataRate => "Data Transfer Rate",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UnitDef {
    pub symbol: &'static str,
    pub name: &'static str,
    pub category: UnitCategory,
    pub factor: f64,
    pub offset: f64,
}

impl UnitDef {
    const fn new(
        symbol: &'static str,
        name: &'static str,
        category: UnitCategory,
        factor: f64,
        offset: f64,
    ) -> Self {
        Self {
            symbol,
            name,
            category,
            factor,
            offset,
        }
    }
}

pub(crate) fn lookup(name: &str) -> Option<UnitDef> {
    by_name().get(name).copied()
}

pub(crate) fn convert(amount: f64, from: UnitDef, to: UnitDef) -> f64 {
    (amount * from.factor + from.offset - to.offset) / to.factor
}

/// Keyword-less counterpart per canonical symbol; only `m→ft` is compound.
pub(crate) fn auto_target(symbol: &str) -> Option<(&'static str, bool)> {
    Some(match symbol {
        "mm" => ("in", false),
        "cm" => ("in", false),
        "m" => ("ft", true),
        "km" => ("mi", false),
        "in" => ("cm", false),
        "ft" => ("m", false),
        "yd" => ("m", false),
        "mi" => ("km", false),
        "mg" => ("g", false),
        "g" => ("oz", false),
        "kg" => ("lb", false),
        "oz" => ("g", false),
        "lb" => ("kg", false),
        "°C" => ("f", false),
        "°F" => ("c", false),
        "K" => ("c", false),
        "ms" => ("s", false),
        "s" => ("ms", false),
        "min" => ("s", false),
        "hr" => ("min", false),
        "day" => ("hr", false),
        "week" => ("day", false),
        "mm²" => ("in2", false),
        "cm²" => ("in2", false),
        "m²" => ("ft2", false),
        "km²" => ("mi2", false),
        "in²" => ("cm2", false),
        "ft²" => ("m2", false),
        "yd²" => ("m2", false),
        "mi²" => ("km2", false),
        "acre" => ("m2", false),
        "ha" => ("acre", false),
        "mL" => ("floz", false),
        "L" => ("gal", false),
        "cup" => ("ml", false),
        "tbsp" => ("ml", false),
        "tsp" => ("ml", false),
        "gal" => ("l", false),
        "qt" => ("l", false),
        "pt" => ("ml", false),
        "fl oz" => ("ml", false),
        "bit" => ("b", false),
        "B" => ("bit", false),
        "kB" => ("kib", false),
        "MB" => ("mib", false),
        "GB" => ("gib", false),
        "TB" => ("tib", false),
        "PB" => ("tb", false),
        "KiB" => ("kb", false),
        "MiB" => ("mb", false),
        "GiB" => ("gb", false),
        "TiB" => ("tb", false),
        "deg" => ("rad", false),
        "rad" => ("deg", false),
        "grad" => ("deg", false),
        "turn" => ("deg", false),
        "arcmin" => ("deg", false),
        "arcsec" => ("deg", false),
        "km/h" => ("mph", false),
        "mph" => ("kmh", false),
        "m/s" => ("kmh", false),
        "kn" => ("kmh", false),
        "ft/s" => ("mph", false),
        "bar" => ("psi", false),
        "psi" => ("bar", false),
        "atm" => ("psi", false),
        "mbar" => ("psi", false),
        "kPa" => ("psi", false),
        "hPa" => ("psi", false),
        "mmHg" => ("psi", false),
        "Torr" => ("psi", false),
        "Mbps" => ("kbps", false),
        "Gbps" => ("mbps", false),
        "Kbps" => ("bps", false),
        "bps" => ("kbps", false),
        "Tbps" => ("gbps", false),
        _ => return None,
    })
}

fn by_name() -> &'static HashMap<String, UnitDef> {
    static TABLE: OnceLock<HashMap<String, UnitDef>> = OnceLock::new();
    TABLE.get_or_init(build_table)
}

fn build_table() -> HashMap<String, UnitDef> {
    let mut table = HashMap::new();
    let mut add = |def: UnitDef, names: &[&str]| {
        for name in names {
            table.insert((*name).to_string(), def);
        }
    };

    add(
        UnitDef::new("mm", "Millimeters", UnitCategory::Length, 0.001, 0.0),
        &["mm", "millimeter", "millimeters", "millimetre", "millimetres"],
    );
    add(
        UnitDef::new("cm", "Centimeters", UnitCategory::Length, 0.01, 0.0),
        &["cm", "centimeter", "centimeters", "centimetre", "centimetres"],
    );
    add(
        UnitDef::new("m", "Meters", UnitCategory::Length, 1.0, 0.0),
        &["m", "meter", "meters", "metre", "metres"],
    );
    add(
        UnitDef::new("km", "Kilometers", UnitCategory::Length, 1000.0, 0.0),
        &["km", "kilometer", "kilometers", "kilometre", "kilometres"],
    );
    add(
        UnitDef::new("in", "Inches", UnitCategory::Length, 0.0254, 0.0),
        &["in", "inch", "inches"],
    );
    add(
        UnitDef::new("ft", "Feet", UnitCategory::Length, 0.3048, 0.0),
        &["ft", "foot", "feet"],
    );
    add(
        UnitDef::new("yd", "Yards", UnitCategory::Length, 0.9144, 0.0),
        &["yd", "yard", "yards"],
    );
    add(
        UnitDef::new("mi", "Miles", UnitCategory::Length, 1609.344, 0.0),
        &["mi", "mile", "miles"],
    );

    add(
        UnitDef::new("mg", "Milligrams", UnitCategory::Weight, 1e-6, 0.0),
        &["mg", "milligram", "milligrams"],
    );
    add(
        UnitDef::new("g", "Grams", UnitCategory::Weight, 0.001, 0.0),
        &["g", "gram", "grams"],
    );
    add(
        UnitDef::new("kg", "Kilograms", UnitCategory::Weight, 1.0, 0.0),
        &["kg", "kilogram", "kilograms", "kilo", "kilos"],
    );
    add(
        UnitDef::new("oz", "Ounces", UnitCategory::Weight, 0.028349523125, 0.0),
        &["oz", "ounce", "ounces"],
    );
    add(
        UnitDef::new("lb", "Pounds", UnitCategory::Weight, 0.45359237, 0.0),
        &["lb", "lbs", "pound", "pounds"],
    );

    add(
        UnitDef::new("°C", "Celsius", UnitCategory::Temperature, 1.0, 273.15),
        &["c", "°c", "celsius", "centigrade"],
    );
    add(
        UnitDef::new(
            "°F",
            "Fahrenheit",
            UnitCategory::Temperature,
            5.0 / 9.0,
            273.15 - 32.0 * 5.0 / 9.0,
        ),
        &["f", "°f", "fahrenheit"],
    );
    add(
        UnitDef::new("K", "Kelvin", UnitCategory::Temperature, 1.0, 0.0),
        &["k", "kelvin", "kelvins"],
    );

    add(
        UnitDef::new("ms", "Milliseconds", UnitCategory::Time, 0.001, 0.0),
        &["ms", "millisecond", "milliseconds"],
    );
    add(
        UnitDef::new("s", "Seconds", UnitCategory::Time, 1.0, 0.0),
        &["s", "sec", "secs", "second", "seconds"],
    );
    add(
        UnitDef::new("min", "Minutes", UnitCategory::Time, 60.0, 0.0),
        &["min", "mins", "minute", "minutes"],
    );
    add(
        UnitDef::new("hr", "Hours", UnitCategory::Time, 3600.0, 0.0),
        &["h", "hr", "hrs", "hour", "hours"],
    );
    add(
        UnitDef::new("day", "Days", UnitCategory::Time, 86400.0, 0.0),
        &["d", "day", "days"],
    );
    add(
        UnitDef::new("week", "Weeks", UnitCategory::Time, 604800.0, 0.0),
        &["wk", "week", "weeks"],
    );

    add(
        UnitDef::new("mm²", "Square Millimeters", UnitCategory::Area, 1e-6, 0.0),
        &["mm2", "sqmm"],
    );
    add(
        UnitDef::new("cm²", "Square Centimeters", UnitCategory::Area, 1e-4, 0.0),
        &["cm2", "sqcm"],
    );
    add(
        UnitDef::new("m²", "Square Meters", UnitCategory::Area, 1.0, 0.0),
        &["m2", "sqm"],
    );
    add(
        UnitDef::new("km²", "Square Kilometers", UnitCategory::Area, 1e6, 0.0),
        &["km2", "sqkm"],
    );
    add(
        UnitDef::new("in²", "Square Inches", UnitCategory::Area, 0.00064516, 0.0),
        &["in2", "sqin"],
    );
    add(
        UnitDef::new("ft²", "Square Feet", UnitCategory::Area, 0.09290304, 0.0),
        &["ft2", "sqft"],
    );
    add(
        UnitDef::new("yd²", "Square Yards", UnitCategory::Area, 0.83612736, 0.0),
        &["yd2", "sqyd"],
    );
    add(
        UnitDef::new("mi²", "Square Miles", UnitCategory::Area, 2_589_988.110336, 0.0),
        &["mi2", "sqmi"],
    );
    add(
        UnitDef::new("acre", "Acres", UnitCategory::Area, 4046.8564224, 0.0),
        &["acre", "acres"],
    );
    add(
        UnitDef::new("ha", "Hectares", UnitCategory::Area, 10000.0, 0.0),
        &["ha", "hectare", "hectares"],
    );

    add(
        UnitDef::new("mL", "Milliliters", UnitCategory::Volume, 0.001, 0.0),
        &["ml", "milliliter", "milliliters", "millilitre", "millilitres"],
    );
    add(
        UnitDef::new("L", "Liters", UnitCategory::Volume, 1.0, 0.0),
        &["l", "liter", "liters", "litre", "litres"],
    );
    add(
        UnitDef::new("cup", "Cups", UnitCategory::Volume, 0.2365882365, 0.0),
        &["cup", "cups"],
    );
    add(
        UnitDef::new("tbsp", "Tablespoons", UnitCategory::Volume, 0.01478676478125, 0.0),
        &["tbsp", "tablespoon", "tablespoons"],
    );
    add(
        UnitDef::new("tsp", "Teaspoons", UnitCategory::Volume, 0.00492892159375, 0.0),
        &["tsp", "teaspoon", "teaspoons"],
    );
    add(
        UnitDef::new("gal", "Gallons", UnitCategory::Volume, 3.785411784, 0.0),
        &["gal", "gallon", "gallons"],
    );
    add(
        UnitDef::new("qt", "Quarts", UnitCategory::Volume, 0.946352946, 0.0),
        &["qt", "quart", "quarts"],
    );
    add(
        UnitDef::new("pt", "Pints", UnitCategory::Volume, 0.473176473, 0.0),
        &["pt", "pint", "pints"],
    );
    add(
        UnitDef::new("fl oz", "Fluid Ounces", UnitCategory::Volume, 0.0295735295625, 0.0),
        &["floz"],
    );

    add(
        UnitDef::new("bit", "Bits", UnitCategory::DigitalStorage, 0.125, 0.0),
        &["bit", "bits"],
    );
    add(
        UnitDef::new("B", "Bytes", UnitCategory::DigitalStorage, 1.0, 0.0),
        &["b", "byte", "bytes"],
    );
    add(
        UnitDef::new("kB", "Kilobytes", UnitCategory::DigitalStorage, 1e3, 0.0),
        &["kb", "kilobyte", "kilobytes"],
    );
    add(
        UnitDef::new("MB", "Megabytes", UnitCategory::DigitalStorage, 1e6, 0.0),
        &["mb", "megabyte", "megabytes"],
    );
    add(
        UnitDef::new("GB", "Gigabytes", UnitCategory::DigitalStorage, 1e9, 0.0),
        &["gb", "gigabyte", "gigabytes"],
    );
    add(
        UnitDef::new("TB", "Terabytes", UnitCategory::DigitalStorage, 1e12, 0.0),
        &["tb", "terabyte", "terabytes"],
    );
    add(
        UnitDef::new("PB", "Petabytes", UnitCategory::DigitalStorage, 1e15, 0.0),
        &["pb", "petabyte", "petabytes"],
    );
    add(
        UnitDef::new("KiB", "Kibibytes", UnitCategory::DigitalStorage, 1024.0, 0.0),
        &["kib", "kibibyte", "kibibytes"],
    );
    add(
        UnitDef::new("MiB", "Mebibytes", UnitCategory::DigitalStorage, 1_048_576.0, 0.0),
        &["mib", "mebibyte", "mebibytes"],
    );
    add(
        UnitDef::new("GiB", "Gibibytes", UnitCategory::DigitalStorage, 1_073_741_824.0, 0.0),
        &["gib", "gibibyte", "gibibytes"],
    );
    add(
        UnitDef::new(
            "TiB",
            "Tebibytes",
            UnitCategory::DigitalStorage,
            1_099_511_627_776.0,
            0.0,
        ),
        &["tib", "tebibyte", "tebibytes"],
    );

    let pi = std::f64::consts::PI;
    add(
        UnitDef::new("rad", "Radians", UnitCategory::Angle, 1.0, 0.0),
        &["rad", "radian", "radians"],
    );
    add(
        UnitDef::new("deg", "Degrees", UnitCategory::Angle, pi / 180.0, 0.0),
        &["deg", "degree", "degrees"],
    );
    add(
        UnitDef::new("grad", "Gradians", UnitCategory::Angle, pi / 200.0, 0.0),
        &["grad", "grads", "gradian", "gradians", "gon"],
    );
    add(
        UnitDef::new("arcmin", "Arcminutes", UnitCategory::Angle, pi / 10800.0, 0.0),
        &["arcmin", "arcminute", "arcminutes"],
    );
    add(
        UnitDef::new("arcsec", "Arcseconds", UnitCategory::Angle, pi / 648000.0, 0.0),
        &["arcsec", "arcsecond", "arcseconds"],
    );
    add(
        UnitDef::new("turn", "Turns", UnitCategory::Angle, 2.0 * pi, 0.0),
        &["turn", "turns", "rev", "revolution", "revolutions"],
    );

    add(
        UnitDef::new("m/s", "Meters per Second", UnitCategory::Speed, 1.0, 0.0),
        &["mps"],
    );
    add(
        UnitDef::new(
            "km/h",
            "Kilometers per Hour",
            UnitCategory::Speed,
            1000.0 / 3600.0,
            0.0,
        ),
        &["kmh", "kph"],
    );
    add(
        UnitDef::new(
            "mph",
            "Miles per Hour",
            UnitCategory::Speed,
            1609.344 / 3600.0,
            0.0,
        ),
        &["mph"],
    );
    add(
        UnitDef::new("ft/s", "Feet per Second", UnitCategory::Speed, 0.3048, 0.0),
        &["fps"],
    );
    add(
        UnitDef::new("kn", "Knots", UnitCategory::Speed, 1852.0 / 3600.0, 0.0),
        &["kn", "knot", "knots"],
    );

    add(
        UnitDef::new("Pa", "Pascals", UnitCategory::Pressure, 1.0, 0.0),
        &["pa", "pascal", "pascals"],
    );
    add(
        UnitDef::new("hPa", "Hectopascals", UnitCategory::Pressure, 100.0, 0.0),
        &["hpa"],
    );
    add(
        UnitDef::new("kPa", "Kilopascals", UnitCategory::Pressure, 1000.0, 0.0),
        &["kpa"],
    );
    add(
        UnitDef::new("bar", "Bar", UnitCategory::Pressure, 100000.0, 0.0),
        &["bar", "bars"],
    );
    add(
        UnitDef::new("mbar", "Millibar", UnitCategory::Pressure, 100.0, 0.0),
        &["mbar", "millibar", "millibars"],
    );
    add(
        UnitDef::new("psi", "PSI", UnitCategory::Pressure, 6894.757293168, 0.0),
        &["psi"],
    );
    add(
        UnitDef::new("atm", "Atmospheres", UnitCategory::Pressure, 101325.0, 0.0),
        &["atm", "atmosphere", "atmospheres"],
    );
    add(
        UnitDef::new("mmHg", "Millimeters of Mercury", UnitCategory::Pressure, 133.322387415, 0.0),
        &["mmhg"],
    );
    add(
        UnitDef::new("Torr", "Torr", UnitCategory::Pressure, 101325.0 / 760.0, 0.0),
        &["torr"],
    );

    add(
        UnitDef::new("bps", "Bits per Second", UnitCategory::DataRate, 1.0, 0.0),
        &["bps"],
    );
    add(
        UnitDef::new("Kbps", "Kilobits per Second", UnitCategory::DataRate, 1e3, 0.0),
        &["kbps"],
    );
    add(
        UnitDef::new("Mbps", "Megabits per Second", UnitCategory::DataRate, 1e6, 0.0),
        &["mbps"],
    );
    add(
        UnitDef::new("Gbps", "Gigabits per Second", UnitCategory::DataRate, 1e9, 0.0),
        &["gbps"],
    );
    add(
        UnitDef::new("Tbps", "Terabits per Second", UnitCategory::DataRate, 1e12, 0.0),
        &["tbps"],
    );

    table
}

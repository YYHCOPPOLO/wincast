package calculator

import (
	"math"
	"sync"
)

type UnitCategory uint8

const (
	Length UnitCategory = iota
	Weight
	Temperature
	Time
	Area
	Volume
	DigitalStorage
	Angle
	Speed
	Pressure
	DataRate
)

func (c UnitCategory) DisplayName() string {
	switch c {
	case Length:
		return "Length"
	case Weight:
		return "Weight"
	case Temperature:
		return "Temperature"
	case Time:
		return "Time"
	case Area:
		return "Area"
	case Volume:
		return "Volume"
	case DigitalStorage:
		return "Digital Storage"
	case Angle:
		return "Angle"
	case Speed:
		return "Speed"
	case Pressure:
		return "Pressure"
	case DataRate:
		return "Data Transfer Rate"
	}
	return "Unknown"
}

type UnitDef struct {
	Symbol   string
	Name     string
	Category UnitCategory
	Factor   float64
	Offset   float64
}

func LookupUnit(name string) (UnitDef, bool) {
	unit, ok := unitTable()[name]
	return unit, ok
}

func ConvertUnit(amount float64, from, to UnitDef) float64 {
	return (amount*from.Factor + from.Offset - to.Offset) / to.Factor
}

// AutoTarget is the keyword-less counterpart for a canonical symbol.
func AutoTarget(symbol string) (string, bool, bool) {
	target, ok := autoTargets[symbol]
	if !ok {
		return "", false, false
	}
	return target.name, target.compound, true
}

type autoTarget struct {
	name     string
	compound bool
}

var autoTargets = map[string]autoTarget{
	"mm": {"in", false}, "cm": {"in", false}, "m": {"ft", true}, "km": {"mi", false},
	"in": {"cm", false}, "ft": {"m", false}, "yd": {"m", false}, "mi": {"km", false},
	"mg": {"g", false}, "g": {"oz", false}, "kg": {"lb", false}, "oz": {"g", false}, "lb": {"kg", false},
	"°C": {"f", false}, "°F": {"c", false}, "K": {"c", false},
	"ms": {"s", false}, "s": {"ms", false}, "min": {"s", false}, "hr": {"min", false},
	"day": {"hr", false}, "week": {"day", false},
	"mm²": {"in2", false}, "cm²": {"in2", false}, "m²": {"ft2", false}, "km²": {"mi2", false},
	"in²": {"cm2", false}, "ft²": {"m2", false}, "yd²": {"m2", false}, "mi²": {"km2", false},
	"acre": {"m2", false}, "ha": {"acre", false},
	"mL": {"floz", false}, "L": {"gal", false}, "cup": {"ml", false}, "tbsp": {"ml", false},
	"tsp": {"ml", false}, "gal": {"l", false}, "qt": {"l", false}, "pt": {"ml", false}, "fl oz": {"ml", false},
	"bit": {"b", false}, "B": {"bit", false}, "kB": {"kib", false}, "MB": {"mib", false},
	"GB": {"gib", false}, "TB": {"tib", false}, "PB": {"tb", false}, "KiB": {"kb", false},
	"MiB": {"mb", false}, "GiB": {"gb", false}, "TiB": {"tb", false},
	"deg": {"rad", false}, "rad": {"deg", false}, "grad": {"deg", false}, "turn": {"deg", false},
	"arcmin": {"deg", false}, "arcsec": {"deg", false},
	"km/h": {"mph", false}, "mph": {"kmh", false}, "m/s": {"kmh", false}, "kn": {"kmh", false}, "ft/s": {"mph", false},
	"bar": {"psi", false}, "psi": {"bar", false}, "atm": {"psi", false}, "mbar": {"psi", false},
	"kPa": {"psi", false}, "hPa": {"psi", false}, "mmHg": {"psi", false}, "Torr": {"psi", false},
	"Mbps": {"kbps", false}, "Gbps": {"mbps", false}, "Kbps": {"bps", false}, "bps": {"kbps", false}, "Tbps": {"gbps", false},
}

var (
	unitOnce  sync.Once
	unitCache map[string]UnitDef
)

func unitTable() map[string]UnitDef {
	unitOnce.Do(func() { unitCache = buildUnitTable() })
	return unitCache
}

func buildUnitTable() map[string]UnitDef {
	table := make(map[string]UnitDef)
	add := func(unit UnitDef, names ...string) {
		for _, name := range names {
			table[name] = unit
		}
	}
	add(UnitDef{"mm", "Millimeters", Length, 0.001, 0}, "mm", "millimeter", "millimeters", "millimetre", "millimetres")
	add(UnitDef{"cm", "Centimeters", Length, 0.01, 0}, "cm", "centimeter", "centimeters", "centimetre", "centimetres")
	add(UnitDef{"m", "Meters", Length, 1, 0}, "m", "meter", "meters", "metre", "metres")
	add(UnitDef{"km", "Kilometers", Length, 1000, 0}, "km", "kilometer", "kilometers", "kilometre", "kilometres")
	add(UnitDef{"in", "Inches", Length, 0.0254, 0}, "in", "inch", "inches")
	add(UnitDef{"ft", "Feet", Length, 0.3048, 0}, "ft", "foot", "feet")
	add(UnitDef{"yd", "Yards", Length, 0.9144, 0}, "yd", "yard", "yards")
	add(UnitDef{"mi", "Miles", Length, 1609.344, 0}, "mi", "mile", "miles")

	add(UnitDef{"mg", "Milligrams", Weight, 1e-6, 0}, "mg", "milligram", "milligrams")
	add(UnitDef{"g", "Grams", Weight, 0.001, 0}, "g", "gram", "grams")
	add(UnitDef{"kg", "Kilograms", Weight, 1, 0}, "kg", "kilogram", "kilograms", "kilo", "kilos")
	add(UnitDef{"oz", "Ounces", Weight, 0.028349523125, 0}, "oz", "ounce", "ounces")
	add(UnitDef{"lb", "Pounds", Weight, 0.45359237, 0}, "lb", "lbs", "pound", "pounds")

	add(UnitDef{"°C", "Celsius", Temperature, 1, 273.15}, "c", "°c", "celsius", "centigrade")
	add(UnitDef{"°F", "Fahrenheit", Temperature, 5.0 / 9.0, 273.15 - 32.0*5.0/9.0}, "f", "°f", "fahrenheit")
	add(UnitDef{"K", "Kelvin", Temperature, 1, 0}, "k", "kelvin", "kelvins")

	add(UnitDef{"ms", "Milliseconds", Time, 0.001, 0}, "ms", "millisecond", "milliseconds")
	add(UnitDef{"s", "Seconds", Time, 1, 0}, "s", "sec", "secs", "second", "seconds")
	add(UnitDef{"min", "Minutes", Time, 60, 0}, "min", "mins", "minute", "minutes")
	add(UnitDef{"hr", "Hours", Time, 3600, 0}, "h", "hr", "hrs", "hour", "hours")
	add(UnitDef{"day", "Days", Time, 86400, 0}, "d", "day", "days")
	add(UnitDef{"week", "Weeks", Time, 604800, 0}, "wk", "week", "weeks")

	add(UnitDef{"mm²", "Square Millimeters", Area, 1e-6, 0}, "mm2", "sqmm")
	add(UnitDef{"cm²", "Square Centimeters", Area, 1e-4, 0}, "cm2", "sqcm")
	add(UnitDef{"m²", "Square Meters", Area, 1, 0}, "m2", "sqm")
	add(UnitDef{"km²", "Square Kilometers", Area, 1e6, 0}, "km2", "sqkm")
	add(UnitDef{"in²", "Square Inches", Area, 0.00064516, 0}, "in2", "sqin")
	add(UnitDef{"ft²", "Square Feet", Area, 0.09290304, 0}, "ft2", "sqft")
	add(UnitDef{"yd²", "Square Yards", Area, 0.83612736, 0}, "yd2", "sqyd")
	add(UnitDef{"mi²", "Square Miles", Area, 2589988.110336, 0}, "mi2", "sqmi")
	add(UnitDef{"acre", "Acres", Area, 4046.8564224, 0}, "acre", "acres")
	add(UnitDef{"ha", "Hectares", Area, 10000, 0}, "ha", "hectare", "hectares")

	add(UnitDef{"mL", "Milliliters", Volume, 0.001, 0}, "ml", "milliliter", "milliliters", "millilitre", "millilitres")
	add(UnitDef{"L", "Liters", Volume, 1, 0}, "l", "liter", "liters", "litre", "litres")
	add(UnitDef{"cup", "Cups", Volume, 0.2365882365, 0}, "cup", "cups")
	add(UnitDef{"tbsp", "Tablespoons", Volume, 0.01478676478125, 0}, "tbsp", "tablespoon", "tablespoons")
	add(UnitDef{"tsp", "Teaspoons", Volume, 0.00492892159375, 0}, "tsp", "teaspoon", "teaspoons")
	add(UnitDef{"gal", "Gallons", Volume, 3.785411784, 0}, "gal", "gallon", "gallons")
	add(UnitDef{"qt", "Quarts", Volume, 0.946352946, 0}, "qt", "quart", "quarts")
	add(UnitDef{"pt", "Pints", Volume, 0.473176473, 0}, "pt", "pint", "pints")
	add(UnitDef{"fl oz", "Fluid Ounces", Volume, 0.0295735295625, 0}, "floz")

	add(UnitDef{"bit", "Bits", DigitalStorage, 0.125, 0}, "bit", "bits")
	add(UnitDef{"B", "Bytes", DigitalStorage, 1, 0}, "b", "byte", "bytes")
	add(UnitDef{"kB", "Kilobytes", DigitalStorage, 1e3, 0}, "kb", "kilobyte", "kilobytes")
	add(UnitDef{"MB", "Megabytes", DigitalStorage, 1e6, 0}, "mb", "megabyte", "megabytes")
	add(UnitDef{"GB", "Gigabytes", DigitalStorage, 1e9, 0}, "gb", "gigabyte", "gigabytes")
	add(UnitDef{"TB", "Terabytes", DigitalStorage, 1e12, 0}, "tb", "terabyte", "terabytes")
	add(UnitDef{"PB", "Petabytes", DigitalStorage, 1e15, 0}, "pb", "petabyte", "petabytes")
	add(UnitDef{"KiB", "Kibibytes", DigitalStorage, 1024, 0}, "kib", "kibibyte", "kibibytes")
	add(UnitDef{"MiB", "Mebibytes", DigitalStorage, 1048576, 0}, "mib", "mebibyte", "mebibytes")
	add(UnitDef{"GiB", "Gibibytes", DigitalStorage, 1073741824, 0}, "gib", "gibibyte", "gibibytes")
	add(UnitDef{"TiB", "Tebibytes", DigitalStorage, 1099511627776, 0}, "tib", "tebibyte", "tebibytes")

	pi := math.Pi
	add(UnitDef{"rad", "Radians", Angle, 1, 0}, "rad", "radian", "radians")
	add(UnitDef{"deg", "Degrees", Angle, pi / 180, 0}, "deg", "degree", "degrees")
	add(UnitDef{"grad", "Gradians", Angle, pi / 200, 0}, "grad", "grads", "gradian", "gradians", "gon")
	add(UnitDef{"arcmin", "Arcminutes", Angle, pi / 10800, 0}, "arcmin", "arcminute", "arcminutes")
	add(UnitDef{"arcsec", "Arcseconds", Angle, pi / 648000, 0}, "arcsec", "arcsecond", "arcseconds")
	add(UnitDef{"turn", "Turns", Angle, 2 * pi, 0}, "turn", "turns", "rev", "revolution", "revolutions")

	add(UnitDef{"m/s", "Meters per Second", Speed, 1, 0}, "mps")
	add(UnitDef{"km/h", "Kilometers per Hour", Speed, 1000.0 / 3600.0, 0}, "kmh", "kph")
	add(UnitDef{"mph", "Miles per Hour", Speed, 1609.344 / 3600.0, 0}, "mph")
	add(UnitDef{"ft/s", "Feet per Second", Speed, 0.3048, 0}, "fps")
	add(UnitDef{"kn", "Knots", Speed, 1852.0 / 3600.0, 0}, "kn", "knot", "knots")

	add(UnitDef{"Pa", "Pascals", Pressure, 1, 0}, "pa", "pascal", "pascals")
	add(UnitDef{"hPa", "Hectopascals", Pressure, 100, 0}, "hpa")
	add(UnitDef{"kPa", "Kilopascals", Pressure, 1000, 0}, "kpa")
	add(UnitDef{"bar", "Bar", Pressure, 100000, 0}, "bar", "bars")
	add(UnitDef{"mbar", "Millibar", Pressure, 100, 0}, "mbar", "millibar", "millibars")
	add(UnitDef{"psi", "PSI", Pressure, 6894.757293168, 0}, "psi")
	add(UnitDef{"atm", "Atmospheres", Pressure, 101325, 0}, "atm", "atmosphere", "atmospheres")
	add(UnitDef{"mmHg", "Millimeters of Mercury", Pressure, 133.322387415, 0}, "mmhg")
	add(UnitDef{"Torr", "Torr", Pressure, 101325.0 / 760.0, 0}, "torr")

	add(UnitDef{"bps", "Bits per Second", DataRate, 1, 0}, "bps")
	add(UnitDef{"Kbps", "Kilobits per Second", DataRate, 1e3, 0}, "kbps")
	add(UnitDef{"Mbps", "Megabits per Second", DataRate, 1e6, 0}, "mbps")
	add(UnitDef{"Gbps", "Gigabits per Second", DataRate, 1e9, 0}, "gbps")
	add(UnitDef{"Tbps", "Terabits per Second", DataRate, 1e12, 0}, "tbps")

	return table
}

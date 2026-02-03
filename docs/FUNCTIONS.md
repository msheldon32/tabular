# Tabular Formula Functions Reference

This document provides a complete reference for all functions available in Tabular's formula system.

## Usage

Formulas are entered in cells starting with `=`. Run `:calc` to evaluate all formulas.

```
=sum(A1:A10)
=avg(B1:B5) * 2
=sqrt(A1^2 + B1^2)
```

All function names are case-insensitive (`SUM`, `Sum`, and `sum` are equivalent).

---

## Aggregate Functions

These functions operate on ranges of cells.

### Basic Aggregates

| Function | Syntax | Description |
|----------|--------|-------------|
| `SUM` | `sum(range)` | Sum of all values |
| `AVG` / `AVERAGE` | `avg(range)` | Arithmetic mean |
| `MIN` | `min(range)` | Minimum value |
| `MAX` | `max(range)` | Maximum value |
| `COUNT` | `count(range)` | Number of cells in range |
| `PRODUCT` | `product(range)` | Product of all values |

**Examples:**
```
=sum(A1:A10)        # Sum of column A, rows 1-10
=avg(A1:E1)         # Average of row 1, columns A-E
=min(A1:C3)         # Minimum in 3x3 block
```

### Central Tendency

| Function | Syntax | Description |
|----------|--------|-------------|
| `MEDIAN` | `median(range)` | Middle value (or average of two middle values) |
| `MODE` | `mode(range)` | Most frequently occurring value |
| `GEOMEAN` | `geomean(range)` | Geometric mean: ⁿ√(x₁ × x₂ × ... × xₙ) |
| `HARMEAN` | `harmean(range)` | Harmonic mean: n / (1/x₁ + 1/x₂ + ... + 1/xₙ) |

**Examples:**
```
=median(A1:A100)    # Middle value of dataset
=geomean(B1:B10)    # Geometric mean (useful for growth rates)
```

### Dispersion & Variability

| Function | Syntax | Description |
|----------|--------|-------------|
| `STDEV` | `stdev(range)` | Sample standard deviation (n-1 denominator) |
| `STDEVP` | `stdevp(range)` | Population standard deviation (n denominator) |
| `VAR` | `var(range)` | Sample variance |
| `VARP` | `varp(range)` | Population variance |
| `AVEDEV` | `avedev(range)` | Average absolute deviation from mean |

**When to use sample vs population:**
- Use `STDEV`/`VAR` when your data is a sample from a larger population
- Use `STDEVP`/`VARP` when your data represents the entire population

**Examples:**
```
=stdev(A1:A50)      # Sample standard deviation
=var(B1:B100)       # Sample variance
```

### Sum of Squares

| Function | Syntax | Description |
|----------|--------|-------------|
| `SUMSQ` | `sumsq(range)` | Sum of squares: Σx² |
| `DEVSQ` | `devsq(range)` | Sum of squared deviations from mean: Σ(x - x̄)² |

**Examples:**
```
=sumsq(A1:A10)      # Sum of x²
=devsq(A1:A10)      # Sum of (x - mean)²
```

### Distribution Shape

| Function | Syntax | Description |
|----------|--------|-------------|
| `SKEW` | `skew(range)` | Skewness (measure of asymmetry) |
| `KURT` | `kurt(range)` | Excess kurtosis (measure of tail weight) |

**Interpreting results:**
- **Skewness**: 0 = symmetric, positive = right-tailed, negative = left-tailed
- **Kurtosis**: 0 = normal distribution, positive = heavy tails, negative = light tails

**Examples:**
```
=skew(A1:A100)      # Test for asymmetry
=kurt(A1:A100)      # Test for heavy/light tails
```

---

## Two-Range Functions

These functions compare or correlate two ranges of equal size.

| Function | Syntax | Description |
|----------|--------|-------------|
| `CORREL` | `correl(range1, range2)` | Pearson correlation coefficient (-1 to 1) |
| `COVAR` | `covar(range1, range2)` | Population covariance |

**Interpreting correlation:**
- 1.0 = perfect positive correlation
- 0.0 = no correlation
- -1.0 = perfect negative correlation

**Examples:**
```
=correl(A1:A20, B1:B20)   # Correlation between columns A and B
=covar(A1:A10, B1:B10)    # Covariance
```

---

## Percentile Functions

| Function | Syntax | Description |
|----------|--------|-------------|
| `PERCENTILE` | `percentile(range, k)` | Value at the k-th percentile (k between 0 and 1) |
| `QUARTILE` | `quartile(range, q)` | Value at quartile q (0=min, 1=Q1, 2=median, 3=Q3, 4=max) |

**Examples:**
```
=percentile(A1:A100, 0.95)   # 95th percentile
=percentile(A1:A100, 0.5)    # 50th percentile (same as median)
=quartile(A1:A100, 1)        # First quartile (25th percentile)
=quartile(A1:A100, 3)        # Third quartile (75th percentile)
```

---

## Mathematical Functions

### Basic Math

| Function | Syntax | Description |
|----------|--------|-------------|
| `ABS` | `abs(x)` | Absolute value |
| `SIGN` | `sign(x)` | Sign of number (-1, 0, or 1) |
| `SQRT` | `sqrt(x)` | Square root |
| `POW` / `POWER` | `pow(x, y)` | x raised to power y |
| `MOD` | `mod(x, y)` | Remainder of x divided by y |

**Examples:**
```
=abs(-5)            # Returns 5
=sqrt(16)           # Returns 4
=pow(2, 10)         # Returns 1024
=mod(17, 5)         # Returns 2
```

### Rounding

| Function | Syntax | Description |
|----------|--------|-------------|
| `FLOOR` | `floor(x)` | Round down to nearest integer |
| `CEIL` | `ceil(x)` | Round up to nearest integer |
| `TRUNC` | `trunc(x)` | Truncate decimal part (round toward zero) |
| `ROUND` | `round(x, digits)` | Round to specified decimal places |

**Examples:**
```
=floor(3.7)         # Returns 3
=ceil(3.2)          # Returns 4
=trunc(-3.7)        # Returns -3
=round(3.14159, 2)  # Returns 3.14
```

### Exponential & Logarithmic

| Function | Syntax | Description |
|----------|--------|-------------|
| `EXP` | `exp(x)` | e raised to power x |
| `LN` | `ln(x)` | Natural logarithm (base e) |
| `LOG10` | `log10(x)` | Logarithm base 10 |
| `LOG2` | `log2(x)` | Logarithm base 2 |
| `LOG` | `log(x, base)` | Logarithm with custom base |

**Examples:**
```
=exp(1)             # Returns e ≈ 2.718
=ln(E())            # Returns 1
=log10(100)         # Returns 2
=log(8, 2)          # Returns 3
```

---

## Trigonometric Functions

All trigonometric functions work in **radians**. Use `RADIANS()` to convert from degrees.

### Basic Trig

| Function | Syntax | Description |
|----------|--------|-------------|
| `SIN` | `sin(x)` | Sine |
| `COS` | `cos(x)` | Cosine |
| `TAN` | `tan(x)` | Tangent |
| `ASIN` | `asin(x)` | Arcsine (inverse sine) |
| `ACOS` | `acos(x)` | Arccosine (inverse cosine) |
| `ATAN` | `atan(x)` | Arctangent (inverse tangent) |
| `ATAN2` | `atan2(y, x)` | Two-argument arctangent |

### Hyperbolic

| Function | Syntax | Description |
|----------|--------|-------------|
| `SINH` | `sinh(x)` | Hyperbolic sine |
| `COSH` | `cosh(x)` | Hyperbolic cosine |
| `TANH` | `tanh(x)` | Hyperbolic tangent |

### Angle Conversion

| Function | Syntax | Description |
|----------|--------|-------------|
| `DEGREES` | `degrees(x)` | Convert radians to degrees |
| `RADIANS` | `radians(x)` | Convert degrees to radians |

**Examples:**
```
=sin(radians(90))   # Returns 1 (sine of 90 degrees)
=cos(PI())          # Returns -1
=degrees(PI())      # Returns 180
=atan2(1, 1)        # Returns π/4 ≈ 0.785
```

---

## Combinatorics

| Function | Syntax | Description |
|----------|--------|-------------|
| `FACT` | `fact(n)` | Factorial: n! |
| `COMBIN` | `combin(n, k)` | Combinations: n choose k = n! / (k!(n-k)!) |
| `PERMUT` | `permut(n, k)` | Permutations: n! / (n-k)! |

**Examples:**
```
=fact(5)            # Returns 120 (5! = 5×4×3×2×1)
=combin(10, 3)      # Returns 120 (ways to choose 3 from 10)
=permut(10, 3)      # Returns 720 (ordered arrangements of 3 from 10)
```

---

## Number Theory

| Function | Syntax | Description |
|----------|--------|-------------|
| `GCD` | `gcd(a, b)` | Greatest common divisor |
| `LCM` | `lcm(a, b)` | Least common multiple |

**Examples:**
```
=gcd(48, 18)        # Returns 6
=lcm(4, 6)          # Returns 12
```

---

## Constants

| Function | Syntax | Value |
|----------|--------|-------|
| `PI` | `PI()` | π ≈ 3.14159265359 |
| `E` | `E()` | e ≈ 2.71828182846 |
| `RAND` | `RAND()` | Random number between 0 and 1 |

**Examples:**
```
=PI()*2             # Returns 2π ≈ 6.283
=E()^2              # Returns e² ≈ 7.389
=RAND()*100         # Random number between 0 and 100
```

---

## Logical & Conditional Functions

These functions enable conditional logic and boolean operations in formulas.

### Conditional Functions

| Function | Syntax | Description |
|----------|--------|-------------|
| `IF` | `if(condition, true_val, false_val)` | Returns `true_val` if condition is true, `false_val` otherwise |
| `IFERROR` | `iferror(value, fallback)` | Returns `value` if valid, `fallback` if error/NaN/Inf |

**Examples:**
```
=if(A1>0, A1, 0)           # Return A1 if positive, else 0
=if(A1>=60, 1, 0)          # Pass/fail: 1 if A1 >= 60
=iferror(A1/B1, 0)         # Safe division, returns 0 on div-by-zero
```

### Boolean Functions

| Function | Syntax | Description |
|----------|--------|-------------|
| `AND` | `and(a, b, ...)` | Returns 1 (true) if ALL arguments are non-zero |
| `OR` | `or(a, b, ...)` | Returns 1 (true) if ANY argument is non-zero |
| `NOT` | `not(x)` | Returns 1 if x is 0, returns 0 otherwise |
| `TRUE` | `true()` | Returns 1 |
| `FALSE` | `false()` | Returns 0 |

**Examples:**
```
=and(A1>0, B1>0)           # True if both A1 and B1 are positive
=or(A1>10, A1<-10)         # True if A1 is outside [-10, 10]
=not(A1=0)                 # True if A1 is not zero
=if(and(A1>=0, A1<=100), A1, 0)  # Clamp to range [0,100]
```

### Boolean Values

Tabular uses numeric boolean representation:
- **True** = 1 (or any non-zero value)
- **False** = 0

Comparison operators return 1 for true, 0 for false.

### Short-Circuit Evaluation

`AND` and `OR` operators use short-circuit evaluation:
- `AND`: Stops evaluating after first false (0) value
- `OR`: Stops evaluating after first true (non-zero) value

This is useful for avoiding errors:
```
=if(B1<>0 AND A1/B1>10, 1, 0)   # Safe: division only happens if B1 is non-zero
```

---

## Cell References

### Single Cells
```
A1      # Column A, row 1
B5      # Column B, row 5
AA100   # Column AA (27th column), row 100
```

### Ranges
```
A1:A10      # Column range (10 cells)
A1:E1       # Row range (5 cells)
A1:C3       # Rectangular range (9 cells)
```

### Entire Row/Column Ranges
```
A:A         # Entire column A
A:C         # Columns A through C
1:1         # Entire row 1
1:5         # Rows 1 through 5
```

**Note:** Entire row/column ranges operate on all data in those rows/columns.

---

## Operators

### Arithmetic Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `+` | Addition | `A1+B1` |
| `-` | Subtraction | `A1-B1` |
| `*` | Multiplication | `A1*2` |
| `/` | Division | `A1/B1` |
| `%` | Modulo | `A1%10` |
| `^` | Power | `A1^2` |

### Comparison Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `=` | Equal to | `A1=B1` |
| `<>` | Not equal to | `A1<>0` |
| `<` | Less than | `A1<10` |
| `<=` | Less than or equal | `A1<=100` |
| `>` | Greater than | `A1>0` |
| `>=` | Greater than or equal | `A1>=B1` |

Comparison operators return 1 (true) or 0 (false).

### Logical Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `AND` | Logical AND (infix) | `A1>0 AND B1>0` |
| `OR` | Logical OR (infix) | `A1<0 OR A1>100` |
| `NOT` | Logical NOT (prefix) | `NOT A1=0` |

Logical operators can be used both as infix operators and as functions:
```
=A1>0 AND B1>0         # Infix style
=and(A1>0, B1>0)       # Function style (equivalent)
```

### Operator Precedence

From highest to lowest:
1. `^` (power)
2. `-` (unary negation), `NOT`
3. `*`, `/`, `%`
4. `+`, `-`
5. `=`, `<>`, `<`, `<=`, `>`, `>=`
6. `AND`
7. `OR`

Use parentheses to override precedence: `=(A1+B1)*C1`

---

## Complex Formula Examples

### Statistics
```
# Coefficient of variation (CV)
=stdev(A1:A100)/avg(A1:A100)*100

# Z-score of a value
=(B1-avg(A1:A100))/stdev(A1:A100)

# Interquartile range (IQR)
=quartile(A1:A100,3)-quartile(A1:A100,1)
```

### Finance
```
# Compound interest: Principal * (1 + rate)^periods
=A1*pow(1+B1,C1)

# Geometric mean of returns
=geomean(A1:A12)
```

### Geometry
```
# Distance between two points (x1,y1) and (x2,y2)
=sqrt(pow(C1-A1,2)+pow(D1-B1,2))

# Area of circle with radius in A1
=PI()*A1^2
```

### Data Analysis
```
# Normalize to 0-1 range
=(A1-min(A1:A100))/(max(A1:A100)-min(A1:A100))

# Percent rank
=percentile(A1:A100,B1/count(A1:A100))
```

### Conditional Logic
```
# Letter grade (simplified)
=if(A1>=90, 4, if(A1>=80, 3, if(A1>=70, 2, if(A1>=60, 1, 0))))

# Clamp value to range [0, 100]
=if(A1<0, 0, if(A1>100, 100, A1))

# Safe division (avoid divide by zero)
=iferror(A1/B1, 0)

# Count values meeting criteria (manual)
=if(A1>0,1,0)+if(A2>0,1,0)+if(A3>0,1,0)

# Boolean flag: is value in range?
=and(A1>=0, A1<=100)

# Exclusive conditions
=if(A1<0, -1, if(A1>0, 1, 0))    # Sign function

# Multiple conditions
=if(and(A1>0, B1>0, C1>0), 1, 0)  # All positive?
=if(or(A1=0, B1=0), 0, A1*B1)     # Multiply if both non-zero
```

---

## Error Handling

Functions return special values for undefined results:

| Value | Meaning |
|-------|---------|
| `NaN` | Not a number (e.g., `sqrt(-1)`, empty dataset) |
| `Inf` | Positive infinity (e.g., division by zero) |
| `-Inf` | Negative infinity |

Circular references are detected and reported as errors.

---

## Notes

- Empty cells are treated as 0 in calculations
- Formulas are evaluated once when `:calc` is run; they are replaced with computed values
- All calculations use 64-bit floating point precision

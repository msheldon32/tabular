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

**Examples:**
```
=median(A1:A100)    # Middle value of dataset
=geomean(B1:B10)    # Geometric mean (useful for growth rates)
```

---

## Mathematical Functions

### Basic Math

| Function | Syntax | Description |
|----------|--------|-------------|
| `ABS` | `abs(x)` | Absolute value |

**Examples:**
```
=abs(-5)            # Returns 5
```

### Rounding

| Function | Syntax | Description |
|----------|--------|-------------|
| `FLOOR` | `floor(x)` | Round down to nearest integer |
| `CEIL` | `ceil(x)` | Round up to nearest integer |

**Examples:**
```
=floor(3.7)         # Returns 3
=ceil(3.2)          # Returns 4
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

# Test Coverage for Metric Instruments

Tests are located in [mod.rs](mod.rs)

// TODO: Fill this correctly.

## Sync Instruments

| Test Type                  | Counter (Delta) | Counter (Cumulative) | UpDownCounter (Delta) | UpDownCounter (Cumulative) | Gauge (Delta) | Gauge (Cumulative) | Histogram (Delta) | Histogram (Cumulative) |
|----------------------------|-----------------|----------------------|----------------------|----------------------------|---------------|--------------------|-------------------|------------------------|
| Regular aggregation test   | :white_check_mark:       | :white_check_mark:             | :x:              | :x:                    | :x:      | :x:            | :x:           | :x:                |
| No-attribute test          | :white_check_mark:        | :white_check_mark:             | :x:              | :x:                    | :x:      | :x:            | :x:           | :x:                |
| Overflow test              | :white_check_mark:        | :white_check_mark:             | :x:              | :x:                    | :x:      | :x:            | :x:           | :x:                |
| Attr Order Sorted First    | :white_check_mark:        | :white_check_mark:             | :x:              | :x:                    | :x:      | :x:            | :x:           | :x:                |
| Attr Order Unsorted First  | :white_check_mark:        | :white_check_mark:             | :x:              | :x:                    | :x:      | :x:            | :x:           | :x:                |

## Observable Instruments

| Test Type                  | ObservableCounter (Delta) | ObservableCounter (Cumulative) | ObservableGauge (Delta) | ObservableGauge (Cumulative) | ObservableUpDownCounter (Delta) | ObservableUpDownCounter (Cumulative) |
|----------------------------|---------------------------|-------------------------------|-------------------------|------------------------------|---------------------------------|--------------------------------------|
| Regular aggregation test    | :x:                  | :x:                      | :x:                | :x:                     | :x:                        | :x:                             |
| No-attribute test           | :x:                  | :x:                      | :x:                | :x:                     | :x:                        | :x:                             |
| Attr Order Sorted First    | :white_check_mark:        | :white_check_mark:             | :x:              | :x:                    | :x:      | :x:            | :x:           | :x:                |
| Attr Order Unsorted First  | :white_check_mark:        | :white_check_mark:             | :x:              | :x:                    | :x:      | :x:            | :x:           | :x:                |
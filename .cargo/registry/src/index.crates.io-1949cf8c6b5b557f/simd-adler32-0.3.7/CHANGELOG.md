# Changelog

## 0.3.3 - 2021-04-14

### Features

- **from_checksum**: add `Adler32::from_checksum`

### Performance Improvements

- **scalar**: improve scalar performance by 90-600%
  - Defer modulo until right before u16 overflow

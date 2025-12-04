#!/usr/bin/env pwsh

& cmake -S ktreesitter -B ktreesitter/.cmake/build `
        -DCMAKE_VERBOSE_MAKEFILE=ON `
        -DCMAKE_INSTALL_PREFIX=ktreesitter/src/jvmMain/resources `
        -DCMAKE_INSTALL_BINDIR="$env:CMAKE_INSTALL_LIBDIR"
& cmake --build ktreesitter/.cmake/build --config Debug
& cmake --install ktreesitter/.cmake/build --config Debug

foreach ($dir in Get-ChildItem -Directory -Path languages) {
    & cmake -S "$dir/build/generated" -B "$dir/.cmake/build" `
            -DCMAKE_INSTALL_PREFIX="$dir/build/generated/src/jvmMain/resources" `
            -DCMAKE_INSTALL_BINDIR="$env:CMAKE_INSTALL_LIBDIR"
    & cmake --build "$dir/.cmake/build" --config Debug
    & cmake --install "$dir/.cmake/build" --config Debug
}

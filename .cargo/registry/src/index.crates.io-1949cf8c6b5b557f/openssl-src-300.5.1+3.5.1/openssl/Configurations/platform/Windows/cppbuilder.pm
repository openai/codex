package platform::Windows::cppbuilder;

use vars qw(@ISA);

require platform::Windows::MSVC;
@ISA = qw(platform::Windows::MSVC);

sub pdbext              { '.tds' }

# C++Builder's Clang-based compilers prepend an underscore to __cdecl-convention
# C functions, and the linker needs those as the InternalName in the .def file.
sub export2internal {
    return "_$_[1]";
}

1;

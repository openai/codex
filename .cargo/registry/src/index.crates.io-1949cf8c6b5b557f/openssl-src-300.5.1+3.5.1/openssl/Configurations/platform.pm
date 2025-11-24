package platform;

use strict;
use warnings;
use vars qw(@ISA);

# Callers must make sure @INC has the build directory
use configdata;

my $module = $target{perl_platform} || 'Unix';
(my $module_path = $module) =~ s|::|/|g;

require "platform/$module_path.pm";
@ISA = ("platform::$module");

1;

__END__

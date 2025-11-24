package OpenSSL::Glob;

use strict;
use warnings;

use File::Glob;

use Exporter;
use vars qw($VERSION @ISA @EXPORT);

$VERSION = '0.1';
@ISA = qw(Exporter);
@EXPORT = qw(glob);

sub glob {
    goto &File::Glob::bsd_glob if $^O ne "VMS";
    goto &CORE::glob;
}

1;
__END__

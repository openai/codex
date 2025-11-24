package gentemplate;

use strict;
use warnings;
use Carp;

use Exporter;
use vars qw($VERSION @ISA @EXPORT @EXPORT_OK %EXPORT_TAGS);
@ISA = qw(Exporter);
@EXPORT = qw(gentemplate);

use File::Basename;

sub gentemplate {
    my %opts = @_;

    my $generator = OpenSSL::GenTemplate->new(%opts);

    # Build mandatory header file generators
    foreach (@{$generator->{info}->{depends}->{""}}) { $generator->dogenerate($_); }

    # Build all known targets, libraries, modules, programs and scripts.
    # Everything else will be handled as a consequence.
    foreach (@{$generator->{info}->{targets}})   { $generator->dotarget($_); }
    foreach (@{$generator->{info}->{libraries}}) { $generator->dolib($_);    }
    foreach (@{$generator->{info}->{modules}})   { $generator->domodule($_); }
    foreach (@{$generator->{info}->{programs}})  { $generator->dobin($_);    }
    foreach (@{$generator->{info}->{scripts}})   { $generator->doscript($_); }
    foreach (sort keys %{$generator->{info}->{htmldocs}}) { $generator->dodocs('html', $_); }
    foreach (sort keys %{$generator->{info}->{mandocs}})  { $generator->dodocs('man', $_); }
    foreach (sort keys %{$generator->{info}->{dirinfo}})  { $generator->dodir($_); }
}

package OpenSSL::GenTemplate;

use OpenSSL::Util;

sub new {
    my $class = shift;
    my %opts = @_;

    my $data = {
        output   => $opts{output},
        config   => $opts{config} // {},
        disabled => $opts{disabled} // {},
        info     => $opts{unified_info} // {},
    };

    return bless $data, $class;
};

sub emit {
    my $self = shift;
    my $name = shift;
    my %opts = @_;
    my $fh = $self->{output};

    die "No name?" unless $name;
    print $fh "{-\n ", $name, '(', dump_data(\%opts), ');', " \n-}"
        unless defined $opts{attrs}->{skip};
}

my $debug_resolvedepends = $ENV{BUILDFILE_DEBUG_DEPENDS};
my $debug_rules = $ENV{BUILDFILE_DEBUG_RULES};

# A cache of objects for which a recipe has already been generated
our %cache;

# collectdepends, expanddepends and reducedepends work together to make
# sure there are no duplicate or weak dependencies and that they are in
# the right order.  This is used to sort the list of libraries  that a
# build depends on.
sub extensionlesslib {
    my @result = map { $_ =~ /(\.a)?$/; $` } @_;
    return @result if wantarray;
    return $result[0];
}

# collectdepends dives into the tree of dependencies and returns
# a list of all the non-weak ones.
sub collectdepends {
    my $self = shift;
    return () unless @_;

    my $thing = shift;
    my $extensionlessthing = extensionlesslib($thing);
    my @listsofar = @_;    # to check if we're looping
    my @list = @{ $self->{info}->{depends}->{$thing} //
                  $self->{info}->{depends}->{$extensionlessthing}
                  // [] };
    my @newlist = ();

    print STDERR "DEBUG[collectdepends] $thing > ", join(' ', @listsofar), "\n"
        if $debug_resolvedepends;
    foreach my $item (@list) {
        my $extensionlessitem = extensionlesslib($item);
        # It's time to break off when the dependency list starts looping
        next if grep { extensionlesslib($_) eq $extensionlessitem } @listsofar;
        # Don't add anything here if the dependency is weak
        next if defined $self->{info}->{attributes}->{depends}->{$thing}->{$item}->{'weak'};
        my @resolved = $self->collectdepends($item, @listsofar, $item);
        push @newlist, $item, @resolved;
    }
    print STDERR "DEBUG[collectdepends] $thing < ", join(' ', @newlist), "\n"
        if $debug_resolvedepends;
    @newlist;
}

# expanddepends goes through a list of stuff, checks if they have any
# dependencies, and adds them at the end of the current position if
# they aren't already present later on.
sub expanddepends {
    my $self = shift;
    my @after = ( @_ );
    print STDERR "DEBUG[expanddepends]> ", join(' ', @after), "\n"
        if $debug_resolvedepends;
    my @before = ();
    while (@after) {
        my $item = shift @after;
        print STDERR "DEBUG[expanddepends]\\  ", join(' ', @before), "\n"
            if $debug_resolvedepends;
        print STDERR "DEBUG[expanddepends] - ", $item, "\n"
            if $debug_resolvedepends;
        my @middle = (
            $item,
            map {
                my $x = $_;
                my $extlessx = extensionlesslib($x);
                if (grep { $extlessx eq extensionlesslib($_) } @before
                    and
                    !grep { $extlessx eq extensionlesslib($_) } @after) {
                    print STDERR "DEBUG[expanddepends] + ", $x, "\n"
                        if $debug_resolvedepends;
                    ( $x )
                } else {
                    print STDERR "DEBUG[expanddepends] ! ", $x, "\n"
                        if $debug_resolvedepends;
                    ()
                }
            } @{$self->{info}->{depends}->{$item} // []}
            );
        print STDERR "DEBUG[expanddepends] = ", join(' ', @middle), "\n"
            if $debug_resolvedepends;
        print STDERR "DEBUG[expanddepends]/  ", join(' ', @after), "\n"
            if $debug_resolvedepends;
        push @before, @middle;
    }
    print STDERR "DEBUG[expanddepends]< ", join(' ', @before), "\n"
        if $debug_resolvedepends;
    @before;
}

# reducedepends looks through a list, and checks if each item is
# repeated later on.  If it is, the earlier copy is dropped.
sub reducedepends {
    my @list = @_;
    print STDERR "DEBUG[reducedepends]> ", join(' ', @list), "\n"
        if $debug_resolvedepends;
    my @newlist = ();
    my %replace = ();
    while (@list) {
        my $item = shift @list;
        my $extensionlessitem = extensionlesslib($item);
        if (grep { $extensionlessitem eq extensionlesslib($_) } @list) {
            if ($item ne $extensionlessitem) {
                # If this instance of the library is explicitly static, we
                # prefer that to any shared library name, since it must have
                # been done on purpose.
                $replace{$extensionlessitem} = $item;
            }
        } else {
            push @newlist, $item;
        }
    }
    @newlist = map { $replace{$_} // $_; } @newlist;
    print STDERR "DEBUG[reducedepends]< ", join(' ', @newlist), "\n"
        if $debug_resolvedepends;
    @newlist;
}

# Do it all
# This takes multiple inputs and combine them into a single list of
# interdependent things.  The returned value will include all the input.
# Callers are responsible for taking away the things they are building.
sub resolvedepends {
    my $self = shift;
    print STDERR "DEBUG[resolvedepends] START (", join(', ', @_), ")\n"
        if $debug_resolvedepends;
    my @all =
        reducedepends($self->expanddepends(map { ( $_, $self->collectdepends($_) ) } @_));
    print STDERR "DEBUG[resolvedepends] END (", join(', ', @_), ") : ",
        join(',', map { "\n    $_" } @all), "\n"
        if $debug_resolvedepends;
    @all;
}

# dogenerate is responsible for producing all the recipes that build
# generated source files.  It recurses in case a dependency is also a
# generated source file.
sub dogenerate {
    my $self = shift;
    my $src = shift;
    # Safety measure
    return "" unless defined $self->{info}->{generate}->{$src};
    return "" if $cache{$src};
    my $obj = shift;
    my $bin = shift;
    my %opts = @_;
    if ($self->{info}->{generate}->{$src}) {
        die "$src is generated by Configure, should not appear in build file\n"
            if ref $self->{info}->{generate}->{$src} eq "";
        my $script = $self->{info}->{generate}->{$src}->[0];
        my %attrs = %{$self->{info}->{attributes}->{generate}->{$src} // {}};
        $self->emit('generatesrc',
             src => $src,
             product => $bin,
             generator => $self->{info}->{generate}->{$src},
             generator_incs => $self->{info}->{includes}->{$script} // [],
             generator_deps => $self->{info}->{depends}->{$script} // [],
             deps => $self->{info}->{depends}->{$src} // [],
             incs => [ defined $obj ? @{$self->{info}->{includes}->{$obj} // []} : (),
                       defined $bin ? @{$self->{info}->{includes}->{$bin} // []} : () ],
             defs => [ defined $obj ? @{$self->{info}->{defines}->{$obj} // []} : (),
                       defined $bin ? @{$self->{info}->{defines}->{$bin} // []} : () ],
             attrs => { %attrs },
             %opts);
        foreach (@{$self->{info}->{depends}->{$src} // []}) {
            $self->dogenerate($_, $obj, $bin, %opts);
        }
        # The generator itself may be is generated
        if ($self->{info}->{generate}->{$script}) {
            $self->dogenerate($script, $obj, $bin, %opts);
        }
    }
    $cache{$src} = 1;
}

sub dotarget {
    my $self = shift;
    my $target = shift;
    return "" if $cache{$target};
    $self->emit('generatetarget',
         target => $target,
         deps => $self->{info}->{depends}->{$target} // []);
    foreach (@{$self->{info}->{depends}->{$target} // []}) {
        $self->dogenerate($_);
    }
    $cache{$target} = 1;
}

# doobj is responsible for producing all the recipes that build
# object files as well as dependency files.
sub doobj {
    my $self = shift;
    my $obj = shift;
    return "" if $cache{$obj};
    my $bin = shift;
    my %opts = @_;
    if (@{$self->{info}->{sources}->{$obj} // []}) {
        my @srcs = @{$self->{info}->{sources}->{$obj}};
        my @deps = @{$self->{info}->{depends}->{$obj} // []};
        my @incs = ( @{$self->{info}->{includes}->{$obj} // []},
                     @{$self->{info}->{includes}->{$bin} // []} );
        my @defs = ( @{$self->{info}->{defines}->{$obj} // []},
                     @{$self->{info}->{defines}->{$bin} // []} );
        print STDERR "DEBUG[doobj] \@srcs for $obj ($bin) : ",
            join(",", map { "\n    $_" } @srcs), "\n"
            if $debug_rules;
        print STDERR "DEBUG[doobj] \@deps for $obj ($bin) : ",
            join(",", map { "\n    $_" } @deps), "\n"
            if $debug_rules;
        print STDERR "DEBUG[doobj] \@incs for $obj ($bin) : ",
            join(",", map { "\n    $_" } @incs), "\n"
            if $debug_rules;
        print STDERR "DEBUG[doobj] \@defs for $obj ($bin) : ",
            join(",", map { "\n    $_" } @defs), "\n"
            if $debug_rules;
        print STDERR "DEBUG[doobj] \%opts for $obj ($bin) : ", ,
            join(",", map { "\n    $_ = $opts{$_}" } sort keys %opts), "\n"
            if $debug_rules;
        $self->emit('src2obj',
             obj => $obj, product => $bin,
             srcs => [ @srcs ], deps => [ @deps ],
             incs => [ @incs ], defs => [ @defs ],
             %opts);
        foreach ((@{$self->{info}->{sources}->{$obj}},
                  @{$self->{info}->{depends}->{$obj} // []})) {
            $self->dogenerate($_, $obj, $bin, %opts);
        }
    }
    $cache{$obj} = 1;
}

# Helper functions to grab all applicable intermediary files.
# This is particularly useful when a library is given as source
# rather than a dependency.  In that case, we consider it to be a
# container with object file references, or possibly references
# to further libraries to pilfer in the same way.
sub getsrclibs {
    my $self = shift;
    my $section = shift;

    # For all input, see if it sources static libraries.  If it does,
    # return them together with the result of a recursive call.
    map { ( $_, getsrclibs($section, $_) ) }
    grep { $_ =~ m|\.a$| }
    map { @{$self->{info}->{$section}->{$_} // []} }
    @_;
}

sub getlibobjs {
    my $self = shift;
    my $section = shift;

    # For all input, see if it's an intermediary file (library or object).
    # If it is, collect the result of a recursive call, or if that returns
    # an empty list, the element itself.  Return the result.
    map {
        my @x = $self->getlibobjs($section, @{$self->{info}->{$section}->{$_}});
        @x ? @x : ( $_ );
    }
    grep { defined $self->{info}->{$section}->{$_} }
    @_;
}

# dolib is responsible for building libraries.  It will call
# obj2shlib if shared libraries are produced, and obj2lib in all
# cases.  It also makes sure all object files for the library are
# built.
sub dolib {
    my $self = shift;
    my $lib = shift;
    return "" if $cache{$lib};

    my %attrs = %{$self->{info}->{attributes}->{libraries}->{$lib} // {}};

    my @deps = ( $self->resolvedepends(getsrclibs('sources', $lib)) );

    # We support two types of objs, those who are specific to this library
    # (they end up in @objs) and those that we get indirectly, via other
    # libraries (they end up in @foreign_objs).  We get the latter any time
    # someone has done something like this in build.info:
    #     SOURCE[libfoo.a]=libbar.a
    # The indirect object files must be kept in a separate array so they
    # don't get rebuilt unnecessarily (and with incorrect auxiliary
    # information).
    #
    # Object files can't be collected commonly for shared and static
    # libraries, because we contain their respective object files in
    # {shared_sources} and {sources}, and because the implications are
    # slightly different for each library form.
    #
    # We grab all these "foreign" object files recursively with getlibobjs().

    unless ($self->{disabled}->{shared} || $lib =~ /\.a$/) {
        # If this library sources other static libraries and those
        # libraries are marked {noinst}, there's no need to include
        # all of their object files.  Instead, we treat those static
        # libraries as dependents alongside any other library this
        # one depends on, and let symbol resolution do its job.
        my @sourced_libs = ();
        my @objs = ();
        my @foreign_objs = ();
        my @deps = ();
        foreach (@{$self->{info}->{shared_sources}->{$lib} // []}) {
            if ($_ !~ m|\.a$|) {
                push @objs, $_;
            } elsif ($self->{info}->{attributes}->{libraries}->{$_}->{noinst}) {
                push @deps, $_;
            } else {
                push @deps, $self->getsrclibs('sources', $_);
                push @foreign_objs, $self->getlibobjs('sources', $_);
            }
        }
        @deps = ( grep { $_ ne $lib } $self->resolvedepends($lib, @deps) );
        print STDERR "DEBUG[dolib:shlib] \%attrs for $lib : ", ,
            join(",", map { "\n    $_ = $attrs{$_}" } sort keys %attrs), "\n"
            if %attrs && $debug_rules;
        print STDERR "DEBUG[dolib:shlib] \@deps for $lib : ",
            join(",", map { "\n    $_" } @deps), "\n"
            if @deps && $debug_rules;
        print STDERR "DEBUG[dolib:shlib] \@objs for $lib : ",
            join(",", map { "\n    $_" } @objs), "\n"
            if @objs && $debug_rules;
        print STDERR "DEBUG[dolib:shlib] \@foreign_objs for $lib : ",
            join(",", map { "\n    $_" } @foreign_objs), "\n"
            if @foreign_objs && $debug_rules;
        $self->emit('obj2shlib',
             lib => $lib,
             attrs => { %attrs },
             objs => [ @objs, @foreign_objs ],
             deps => [ @deps ]);
        foreach (@objs) {
            # If this is somehow a compiled object, take care of it that way
            # Otherwise, it might simply be generated
            if (defined $self->{info}->{sources}->{$_}) {
                if($_ =~ /\.a$/) {
                    $self->dolib($_);
                } else {
                    $self->doobj($_, $lib, intent => "shlib", attrs => { %attrs });
                }
            } else {
                $self->dogenerate($_, undef, undef, intent => "lib");
            }
        }
    }
    {
        # When putting static libraries together, we cannot rely on any
        # symbol resolution, so for all static libraries used as source for
        # this one, as well as other libraries they depend on, we simply
        # grab all their object files unconditionally,
        # Symbol resolution will happen when any program, module or shared
        # library is linked with this one.
        my @objs = ();
        my @sourcedeps = ();
        my @foreign_objs = ();
        foreach (@{$self->{info}->{sources}->{$lib}}) {
            if ($_ !~ m|\.a$|) {
                push @objs, $_;
            } else {
                push @sourcedeps, $_;
            }
        }
        @sourcedeps = ( grep { $_ ne $lib } $self->resolvedepends(@sourcedeps) );
        print STDERR "DEBUG[dolib:lib] : \@sourcedeps for $_ : ",
            join(",", map { "\n    $_" } @sourcedeps), "\n"
            if @sourcedeps && $debug_rules;
        @foreign_objs = $self->getlibobjs('sources', @sourcedeps);
        print STDERR "DEBUG[dolib:lib] \%attrs for $lib : ", ,
            join(",", map { "\n    $_ = $attrs{$_}" } sort keys %attrs), "\n"
            if %attrs && $debug_rules;
        print STDERR "DEBUG[dolib:lib] \@objs for $lib : ",
            join(",", map { "\n    $_" } @objs), "\n"
            if @objs && $debug_rules;
        print STDERR "DEBUG[dolib:lib] \@foreign_objs for $lib : ",
            join(",", map { "\n    $_" } @foreign_objs), "\n"
            if @foreign_objs && $debug_rules;
        $self->emit('obj2lib',
             lib => $lib, attrs => { %attrs },
             objs => [ @objs, @foreign_objs ]);
        foreach (@objs) {
            $self->doobj($_, $lib, intent => "lib", attrs => { %attrs });
        }
    }
    $cache{$lib} = 1;
}

# domodule is responsible for building modules.  It will call
# obj2dso, and also makes sure all object files for the library
# are built.
sub domodule {
    my $self = shift;
    my $module = shift;
    return "" if $cache{$module};
    my %attrs = %{$self->{info}->{attributes}->{modules}->{$module} // {}};
    my @objs = @{$self->{info}->{sources}->{$module}};
    my @deps = ( grep { $_ ne $module }
                 $self->resolvedepends($module) );
    print STDERR "DEBUG[domodule] \%attrs for $module :",
        join(",", map { "\n    $_ = $attrs{$_}" } sort keys %attrs), "\n"
        if $debug_rules;
    print STDERR "DEBUG[domodule] \@objs for $module : ",
        join(",", map { "\n    $_" } @objs), "\n"
        if $debug_rules;
    print STDERR "DEBUG[domodule] \@deps for $module : ",
        join(",", map { "\n    $_" } @deps), "\n"
        if $debug_rules;
    $self->emit('obj2dso',
         module => $module,
         attrs => { %attrs },
         objs => [ @objs ],
         deps => [ @deps ]);
    foreach (@{$self->{info}->{sources}->{$module}}) {
        # If this is somehow a compiled object, take care of it that way
        # Otherwise, it might simply be generated
        if (defined $self->{info}->{sources}->{$_}) {
            $self->doobj($_, $module, intent => "dso", attrs => { %attrs });
        } else {
            $self->dogenerate($_, undef, $module, intent => "dso");
        }
    }
    $cache{$module} = 1;
}

# dobin is responsible for building programs.  It will call obj2bin,
# and also makes sure all object files for the library are built.
sub dobin {
    my $self = shift;
    my $bin = shift;
    return "" if $cache{$bin};
    my %attrs = %{$self->{info}->{attributes}->{programs}->{$bin} // {}};
    my @objs = @{$self->{info}->{sources}->{$bin}};
    my @deps = ( grep { $_ ne $bin } $self->resolvedepends($bin) );
    print STDERR "DEBUG[dobin] \%attrs for $bin : ",
        join(",", map { "\n    $_ = $attrs{$_}" } sort keys %attrs), "\n"
        if %attrs && $debug_rules;
    print STDERR "DEBUG[dobin] \@objs for $bin : ",
        join(",", map { "\n    $_" } @objs), "\n"
        if @objs && $debug_rules;
    print STDERR "DEBUG[dobin] \@deps for $bin : ",
        join(",", map { "\n    $_" } @deps), "\n"
        if @deps && $debug_rules;
    $self->emit('obj2bin',
         bin => $bin,
         attrs => { %attrs },
         objs => [ @objs ],
         deps => [ @deps ]);
    foreach (@objs) {
        $self->doobj($_, $bin, intent => "bin", attrs => { %attrs });
    }
    $cache{$bin} = 1;
}

# doscript is responsible for building scripts from templates.  It will
# call in2script.
sub doscript {
    my $self = shift;
    my $script = shift;
    return "" if $cache{$script};
    $self->emit('in2script',
         script => $script,
         attrs => $self->{info}->{attributes}->{scripts}->{$script} // {},
         sources => $self->{info}->{sources}->{$script});
    $cache{$script} = 1;
}

sub dodir {
    my $self = shift;
    my $dir = shift;
    return "" if !exists(&generatedir) or $cache{$dir};
    $self->emit('generatedir',
         dir => $dir,
         deps => $self->{info}->{dirinfo}->{$dir}->{deps} // [],
         %{$self->{info}->{dirinfo}->{$_}->{products}});
    $cache{$dir} = 1;
}

# dodocs is responsible for building documentation from .pods.
# It will call generatesrc.
sub dodocs {
    my $self = shift;
    my $type = shift;
    my $section = shift;
    foreach my $doc (@{$self->{info}->{"${type}docs"}->{$section}}) {
        next if $cache{$doc};
        $self->emit('generatesrc',
             src => $doc,
             generator => $self->{info}->{generate}->{$doc});
        foreach ((@{$self->{info}->{depends}->{$doc} // []})) {
            $self->dogenerate($_, undef, undef);
        }
        $cache{$doc} = 1;
    }
}

1;

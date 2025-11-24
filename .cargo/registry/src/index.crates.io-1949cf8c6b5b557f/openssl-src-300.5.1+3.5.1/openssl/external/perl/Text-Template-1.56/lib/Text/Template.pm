# -*- perl -*-
# Text::Template.pm
#
# Fill in `templates'
#
# Copyright 2013 M. J. Dominus.
# You may copy and distribute this program under the
# same terms as Perl itself.
# If in doubt, write to mjd-perl-template+@plover.com for a license.
#

package Text::Template;
$Text::Template::VERSION = '1.56';
# ABSTRACT: Expand template text with embedded Perl

use strict;
use warnings;

require 5.008;

use base 'Exporter';

our @EXPORT_OK = qw(fill_in_file fill_in_string TTerror);
our $ERROR;

my %GLOBAL_PREPEND = ('Text::Template' => '');

sub Version {
    $Text::Template::VERSION;
}

sub _param {
    my ($k, %h) = @_;

    for my $kk ($k, "\u$k", "\U$k", "-$k", "-\u$k", "-\U$k") {
        return $h{$kk} if exists $h{$kk};
    }

    return undef;
}

sub always_prepend {
    my $pack = shift;

    my $old = $GLOBAL_PREPEND{$pack};

    $GLOBAL_PREPEND{$pack} = shift;

    $old;
}

{
    my %LEGAL_TYPE;

    BEGIN {
        %LEGAL_TYPE = map { $_ => 1 } qw(FILE FILEHANDLE STRING ARRAY);
    }

    sub new {
        my ($pack, %a) = @_;

        my $stype     = uc(_param('type', %a) || "FILE");
        my $source    = _param('source', %a);
        my $untaint   = _param('untaint', %a);
        my $prepend   = _param('prepend', %a);
        my $alt_delim = _param('delimiters', %a);
        my $broken    = _param('broken', %a);
        my $encoding  = _param('encoding', %a);

        unless (defined $source) {
            require Carp;
            Carp::croak("Usage: $ {pack}::new(TYPE => ..., SOURCE => ...)");
        }

        unless ($LEGAL_TYPE{$stype}) {
            require Carp;
            Carp::croak("Illegal value `$stype' for TYPE parameter");
        }

        my $self = {
            TYPE     => $stype,
            PREPEND  => $prepend,
            UNTAINT  => $untaint,
            BROKEN   => $broken,
            ENCODING => $encoding,
            (defined $alt_delim ? (DELIM => $alt_delim) : ())
        };

        # Under 5.005_03, if any of $stype, $prepend, $untaint, or $broken
        # are tainted, all the others become tainted too as a result of
        # sharing the expression with them.  We install $source separately
        # to prevent it from acquiring a spurious taint.
        $self->{SOURCE} = $source;

        bless $self => $pack;
        return unless $self->_acquire_data;

        $self;
    }
}

# Convert template objects of various types to type STRING,
# in which the template data is embedded in the object itself.
sub _acquire_data {
    my $self = shift;

    my $type = $self->{TYPE};

    if ($type eq 'STRING') {
        # nothing necessary
    }
    elsif ($type eq 'FILE') {
        my $data = _load_text($self->{SOURCE});
        unless (defined $data) {

            # _load_text already set $ERROR
            return undef;
        }

        if ($self->{UNTAINT} && _is_clean($self->{SOURCE})) {
            _unconditionally_untaint($data);
        }

        if (defined $self->{ENCODING}) {
            require Encode;
            $data = Encode::decode($self->{ENCODING}, $data, &Encode::FB_CROAK);
        }

        $self->{TYPE}     = 'STRING';
        $self->{FILENAME} = $self->{SOURCE};
        $self->{SOURCE}   = $data;
    }
    elsif ($type eq 'ARRAY') {
        $self->{TYPE} = 'STRING';
        $self->{SOURCE} = join '', @{ $self->{SOURCE} };
    }
    elsif ($type eq 'FILEHANDLE') {
        $self->{TYPE} = 'STRING';
        local $/;
        my $fh   = $self->{SOURCE};
        my $data = <$fh>;             # Extra assignment avoids bug in Solaris perl5.00[45].
        if ($self->{UNTAINT}) {
            _unconditionally_untaint($data);
        }
        $self->{SOURCE} = $data;
    }
    else {
        # This should have been caught long ago, so it represents a
        # drastic `can't-happen' sort of failure
        my $pack = ref $self;
        die "Can only acquire data for $pack objects of subtype STRING, but this is $type; aborting";
    }

    $self->{DATA_ACQUIRED} = 1;
}

sub source {
    my $self = shift;

    $self->_acquire_data unless $self->{DATA_ACQUIRED};

    return $self->{SOURCE};
}

sub set_source_data {
    my ($self, $newdata, $type) = @_;

    $self->{SOURCE}        = $newdata;
    $self->{DATA_ACQUIRED} = 1;
    $self->{TYPE}          = $type || 'STRING';

    1;
}

sub compile {
    my $self = shift;

    return 1 if $self->{TYPE} eq 'PREPARSED';

    return undef unless $self->_acquire_data;

    unless ($self->{TYPE} eq 'STRING') {
        my $pack = ref $self;

        # This should have been caught long ago, so it represents a
        # drastic `can't-happen' sort of failure
        die "Can only compile $pack objects of subtype STRING, but this is $self->{TYPE}; aborting";
    }

    my @tokens;
    my $delim_pats = shift() || $self->{DELIM};

    my ($t_open, $t_close) = ('{', '}');
    my $DELIM;    # Regex matches a delimiter if $delim_pats

    if (defined $delim_pats) {
        ($t_open, $t_close) = @$delim_pats;
        $DELIM = "(?:(?:\Q$t_open\E)|(?:\Q$t_close\E))";
        @tokens = split /($DELIM|\n)/, $self->{SOURCE};
    }
    else {
        @tokens = split /(\\\\(?=\\*[{}])|\\[{}]|[{}\n])/, $self->{SOURCE};
    }

    my $state  = 'TEXT';
    my $depth  = 0;
    my $lineno = 1;
    my @content;
    my $cur_item = '';
    my $prog_start;

    while (@tokens) {
        my $t = shift @tokens;

        next if $t eq '';

        if ($t eq $t_open) {    # Brace or other opening delimiter
            if ($depth == 0) {
                push @content, [ $state, $cur_item, $lineno ] if $cur_item ne '';
                $cur_item   = '';
                $state      = 'PROG';
                $prog_start = $lineno;
            }
            else {
                $cur_item .= $t;
            }
            $depth++;
        }
        elsif ($t eq $t_close) {    # Brace or other closing delimiter
            $depth--;
            if ($depth < 0) {
                $ERROR = "Unmatched close brace at line $lineno";
                return undef;
            }
            elsif ($depth == 0) {
                push @content, [ $state, $cur_item, $prog_start ] if $cur_item ne '';
                $state    = 'TEXT';
                $cur_item = '';
            }
            else {
                $cur_item .= $t;
            }
        }
        elsif (!$delim_pats && $t eq '\\\\') {    # precedes \\\..\\\{ or \\\..\\\}
            $cur_item .= '\\';
        }
        elsif (!$delim_pats && $t =~ /^\\([{}])$/) {    # Escaped (literal) brace?
            $cur_item .= $1;
        }
        elsif ($t eq "\n") {                            # Newline
            $lineno++;
            $cur_item .= $t;
        }
        else {                                          # Anything else
            $cur_item .= $t;
        }
    }

    if ($state eq 'PROG') {
        $ERROR = "End of data inside program text that began at line $prog_start";
        return undef;
    }
    elsif ($state eq 'TEXT') {
        push @content, [ $state, $cur_item, $lineno ] if $cur_item ne '';
    }
    else {
        die "Can't happen error #1";
    }

    $self->{TYPE}   = 'PREPARSED';
    $self->{SOURCE} = \@content;

    1;
}

sub prepend_text {
    my $self = shift;

    my $t = $self->{PREPEND};

    unless (defined $t) {
        $t = $GLOBAL_PREPEND{ ref $self };
        unless (defined $t) {
            $t = $GLOBAL_PREPEND{'Text::Template'};
        }
    }

    $self->{PREPEND} = $_[1] if $#_ >= 1;

    return $t;
}

sub fill_in {
    my ($fi_self, %fi_a) = @_;

    unless ($fi_self->{TYPE} eq 'PREPARSED') {
        my $delims = _param('delimiters', %fi_a);
        my @delim_arg = (defined $delims ? ($delims) : ());
        $fi_self->compile(@delim_arg)
            or return undef;
    }

    my $fi_varhash    = _param('hash',       %fi_a);
    my $fi_package    = _param('package',    %fi_a);
    my $fi_broken     = _param('broken',     %fi_a) || $fi_self->{BROKEN} || \&_default_broken;
    my $fi_broken_arg = _param('broken_arg', %fi_a) || [];
    my $fi_safe       = _param('safe',       %fi_a);
    my $fi_ofh        = _param('output',     %fi_a);
    my $fi_filename   = _param('filename',   %fi_a) || $fi_self->{FILENAME} || 'template';
    my $fi_strict     = _param('strict',     %fi_a);
    my $fi_prepend    = _param('prepend',    %fi_a);

    my $fi_eval_package;
    my $fi_scrub_package = 0;

    unless (defined $fi_prepend) {
        $fi_prepend = $fi_self->prepend_text;
    }

    if (defined $fi_safe) {
        $fi_eval_package = 'main';
    }
    elsif (defined $fi_package) {
        $fi_eval_package = $fi_package;
    }
    elsif (defined $fi_varhash) {
        $fi_eval_package  = _gensym();
        $fi_scrub_package = 1;
    }
    else {
        $fi_eval_package = caller;
    }

    my @fi_varlist;
    my $fi_install_package;

    if (defined $fi_varhash) {
        if (defined $fi_package) {
            $fi_install_package = $fi_package;
        }
        elsif (defined $fi_safe) {
            $fi_install_package = $fi_safe->root;
        }
        else {
            $fi_install_package = $fi_eval_package;    # The gensymmed one
        }
        @fi_varlist = _install_hash($fi_varhash => $fi_install_package);
        if ($fi_strict) {
            $fi_prepend = "use vars qw(@fi_varlist);$fi_prepend" if @fi_varlist;
            $fi_prepend = "use strict;$fi_prepend";
        }
    }

    if (defined $fi_package && defined $fi_safe) {
        no strict 'refs';

        # Big fat magic here: Fix it so that the user-specified package
        # is the default one available in the safe compartment.
        *{ $fi_safe->root . '::' } = \%{ $fi_package . '::' };    # LOD
    }

    my $fi_r = '';
    my $fi_item;
    foreach $fi_item (@{ $fi_self->{SOURCE} }) {
        my ($fi_type, $fi_text, $fi_lineno) = @$fi_item;
        if ($fi_type eq 'TEXT') {
            $fi_self->append_text_to_output(
                text   => $fi_text,
                handle => $fi_ofh,
                out    => \$fi_r,
                type   => $fi_type,);
        }
        elsif ($fi_type eq 'PROG') {
            no strict;

            my $fi_lcomment = "#line $fi_lineno $fi_filename";
            my $fi_progtext = "package $fi_eval_package; $fi_prepend;\n$fi_lcomment\n$fi_text;\n;";
            my $fi_res;
            my $fi_eval_err = '';

            if ($fi_safe) {
                no strict;
                no warnings;

                $fi_safe->reval(q{undef $OUT});
                $fi_res      = $fi_safe->reval($fi_progtext);
                $fi_eval_err = $@;
                my $OUT = $fi_safe->reval('$OUT');
                $fi_res = $OUT if defined $OUT;
            }
            else {
                no strict;
                no warnings;

                my $OUT;
                $fi_res      = eval $fi_progtext;
                $fi_eval_err = $@;
                $fi_res      = $OUT if defined $OUT;
            }

            # If the value of the filled-in text really was undef,
            # change it to an explicit empty string to avoid undefined
            # value warnings later.
            $fi_res = '' unless defined $fi_res;

            if ($fi_eval_err) {
                $fi_res = $fi_broken->(
                    text   => $fi_text,
                    error  => $fi_eval_err,
                    lineno => $fi_lineno,
                    arg    => $fi_broken_arg,);
                if (defined $fi_res) {
                    $fi_self->append_text_to_output(
                        text   => $fi_res,
                        handle => $fi_ofh,
                        out    => \$fi_r,
                        type   => $fi_type,);
                }
                else {
                    return $fi_r;    # Undefined means abort processing
                }
            }
            else {
                $fi_self->append_text_to_output(
                    text   => $fi_res,
                    handle => $fi_ofh,
                    out    => \$fi_r,
                    type   => $fi_type,);
            }
        }
        else {
            die "Can't happen error #2";
        }
    }

    _scrubpkg($fi_eval_package) if $fi_scrub_package;

    defined $fi_ofh ? 1 : $fi_r;
}

sub append_text_to_output {
    my ($self, %arg) = @_;

    if (defined $arg{handle}) {
        print { $arg{handle} } $arg{text};
    }
    else {
        ${ $arg{out} } .= $arg{text};
    }

    return;
}

sub fill_this_in {
    my ($pack, $text) = splice @_, 0, 2;

    my $templ = $pack->new(TYPE => 'STRING', SOURCE => $text, @_)
        or return undef;

    $templ->compile or return undef;

    my $result = $templ->fill_in(@_);

    $result;
}

sub fill_in_string {
    my $string = shift;

    my $package = _param('package', @_);

    push @_, 'package' => scalar(caller) unless defined $package;

    Text::Template->fill_this_in($string, @_);
}

sub fill_in_file {
    my $fn = shift;
    my $templ = Text::Template->new(TYPE => 'FILE', SOURCE => $fn, @_) or return undef;

    $templ->compile or return undef;

    my $text = $templ->fill_in(@_);

    $text;
}

sub _default_broken {
    my %a = @_;

    my $prog_text = $a{text};
    my $err       = $a{error};
    my $lineno    = $a{lineno};

    chomp $err;

    #  $err =~ s/\s+at .*//s;
    "Program fragment delivered error ``$err''";
}

sub _load_text {
    my $fn = shift;

    open my $fh, '<', $fn or do {
        $ERROR = "Couldn't open file $fn: $!";
        return undef;
    };

    local $/;

    <$fh>;
}

sub _is_clean {
    my $z;

    eval { ($z = join('', @_)), eval '#' . substr($z, 0, 0); 1 }    # LOD
}

sub _unconditionally_untaint {
    for (@_) {
        ($_) = /(.*)/s;
    }
}

{
    my $seqno = 0;

    sub _gensym {
        __PACKAGE__ . '::GEN' . $seqno++;
    }

    sub _scrubpkg {
        my $s = shift;

        $s =~ s/^Text::Template:://;

        no strict 'refs';

        my $hash = $Text::Template::{ $s . "::" };

        foreach my $key (keys %$hash) {
            undef $hash->{$key};
        }

        %$hash = ();

        delete $Text::Template::{ $s . "::" };
    }
}

# Given a hashful of variables (or a list of such hashes)
# install the variables into the specified package,
# overwriting whatever variables were there before.
sub _install_hash {
    my $hashlist = shift;
    my $dest     = shift;

    if (UNIVERSAL::isa($hashlist, 'HASH')) {
        $hashlist = [$hashlist];
    }

    my @varlist;

    for my $hash (@$hashlist) {
        for my $name (keys %$hash) {
            my $val = $hash->{$name};

            no strict 'refs';
            no warnings 'redefine';

            local *SYM = *{"$ {dest}::$name"};

            if (!defined $val) {
                delete ${"$ {dest}::"}{$name};
                my $match = qr/^.\Q$name\E$/;
                @varlist = grep { $_ !~ $match } @varlist;
            }
            elsif (ref $val) {
                *SYM = $val;
                push @varlist, do {
                    if    (UNIVERSAL::isa($val, 'ARRAY')) { '@' }
                    elsif (UNIVERSAL::isa($val, 'HASH'))  { '%' }
                    else                                  { '$' }
                    }
                    . $name;
            }
            else {
                *SYM = \$val;
                push @varlist, '$' . $name;
            }
        }
    }

    @varlist;
}

sub TTerror { $ERROR }

1;

__END__

=pod

=encoding UTF-8

=head1 NAME

Text::Template - Expand template text with embedded Perl

=head1 VERSION

version 1.56

=head1 SYNOPSIS

 use Text::Template;


 $template = Text::Template->new(TYPE => 'FILE',  SOURCE => 'filename.tmpl');
 $template = Text::Template->new(TYPE => 'ARRAY', SOURCE => [ ... ] );
 $template = Text::Template->new(TYPE => 'FILEHANDLE', SOURCE => $fh );
 $template = Text::Template->new(TYPE => 'STRING', SOURCE => '...' );
 $template = Text::Template->new(PREPEND => q{use strict;}, ...);

 # Use a different template file syntax:
 $template = Text::Template->new(DELIMITERS => [$open, $close], ...);

 $recipient = 'King';
 $text = $template->fill_in();  # Replaces `{$recipient}' with `King'
 print $text;

 $T::recipient = 'Josh';
 $text = $template->fill_in(PACKAGE => T);

 # Pass many variables explicitly
 $hash = { recipient => 'Abed-Nego',
           friends => [ 'me', 'you' ],
           enemies => { loathsome => 'Saruman',
                        fearsome => 'Sauron' },
         };
 $text = $template->fill_in(HASH => $hash, ...);
 # $recipient is Abed-Nego,
 # @friends is ( 'me', 'you' ),
 # %enemies is ( loathsome => ..., fearsome => ... )


 # Call &callback in case of programming errors in template
 $text = $template->fill_in(BROKEN => \&callback, BROKEN_ARG => $ref, ...);

 # Evaluate program fragments in Safe compartment with restricted permissions
 $text = $template->fill_in(SAFE => $compartment, ...);

 # Print result text instead of returning it
 $success = $template->fill_in(OUTPUT => \*FILEHANDLE, ...);

 # Parse template with different template file syntax:
 $text = $template->fill_in(DELIMITERS => [$open, $close], ...);
 # Note that this is *faster* than using the default delimiters

 # Prepend specified perl code to each fragment before evaluating:
 $text = $template->fill_in(PREPEND => q{use strict 'vars';}, ...);

 use Text::Template 'fill_in_string';
 $text = fill_in_string( <<EOM, PACKAGE => 'T', ...);
 Dear {$recipient},
 Pay me at once.
        Love, 
         G.V.
 EOM

 use Text::Template 'fill_in_file';
 $text = fill_in_file($filename, ...);

 # All templates will always have `use strict vars' attached to all fragments
 Text::Template->always_prepend(q{use strict 'vars';});

=head1 DESCRIPTION

This is a library for generating form letters, building HTML pages, or
filling in templates generally.  A `template' is a piece of text that
has little Perl programs embedded in it here and there.  When you
`fill in' a template, you evaluate the little programs and replace
them with their values.  

You can store a template in a file outside your program.  People can
modify the template without modifying the program.  You can separate
the formatting details from the main code, and put the formatting
parts of the program into the template.  That prevents code bloat and
encourages functional separation.

=head2 Example

Here's an example of a template, which we'll suppose is stored in the
file C<formletter.tmpl>:

    Dear {$title} {$lastname},

    It has come to our attention that you are delinquent in your
    {$monthname[$last_paid_month]} payment.  Please remit
    ${sprintf("%.2f", $amount)} immediately, or your patellae may
    be needlessly endangered.

                    Love,

                    Mark "Vizopteryx" Dominus

The result of filling in this template is a string, which might look
something like this:

    Dear Mr. Smith,

    It has come to our attention that you are delinquent in your
    February payment.  Please remit
    $392.12 immediately, or your patellae may
    be needlessly endangered.


                    Love,

                    Mark "Vizopteryx" Dominus

Here is a complete program that transforms the example
template into the example result, and prints it out:

    use Text::Template;

    my $template = Text::Template->new(SOURCE => 'formletter.tmpl')
      or die "Couldn't construct template: $Text::Template::ERROR";

    my @monthname = qw(January February March April May June
                       July August September October November December);
    my %vars = (title           => 'Mr.',
                firstname       => 'John',
                lastname        => 'Smith',
                last_paid_month => 1,   # February
                amount          => 392.12,
                monthname       => \@monthname);

    my $result = $template->fill_in(HASH => \%vars);

    if (defined $result) { print $result }
    else { die "Couldn't fill in template: $Text::Template::ERROR" }

=head2 Philosophy

When people make a template module like this one, they almost always
start by inventing a special syntax for substitutions.  For example,
they build it so that a string like C<%%VAR%%> is replaced with the
value of C<$VAR>.  Then they realize the need extra formatting, so
they put in some special syntax for formatting.  Then they need a
loop, so they invent a loop syntax.  Pretty soon they have a new
little template language.

This approach has two problems: First, their little language is
crippled. If you need to do something the author hasn't thought of,
you lose.  Second: Who wants to learn another language?  You already
know Perl, so why not use it?

C<Text::Template> templates are programmed in I<Perl>.  You embed Perl
code in your template, with C<{> at the beginning and C<}> at the end.
If you want a variable interpolated, you write it the way you would in
Perl.  If you need to make a loop, you can use any of the Perl loop
constructions.  All the Perl built-in functions are available.

=head1 Details

=head2 Template Parsing

The C<Text::Template> module scans the template source.  An open brace
C<{> begins a program fragment, which continues until the matching
close brace C<}>.  When the template is filled in, the program
fragments are evaluated, and each one is replaced with the resulting
value to yield the text that is returned.

A backslash C<\> in front of a brace (or another backslash that is in
front of a brace) escapes its special meaning.  The result of filling
out this template:

    \{ The sum of 1 and 2 is {1+2}  \}

is

    { The sum of 1 and 2 is 3  }

If you have an unmatched brace, C<Text::Template> will return a
failure code and a warning about where the problem is.  Backslashes
that do not precede a brace are passed through unchanged.  If you have
a template like this:

    { "String that ends in a newline.\n" }

The backslash inside the string is passed through to Perl unchanged,
so the C<\n> really does turn into a newline.  See the note at the end
for details about the way backslashes work.  Backslash processing is
I<not> done when you specify alternative delimiters with the
C<DELIMITERS> option.  (See L<"Alternative Delimiters">, below.)

Each program fragment should be a sequence of Perl statements, which
are evaluated the usual way.  The result of the last statement
executed will be evaluated in scalar context; the result of this
statement is a string, which is interpolated into the template in
place of the program fragment itself.

The fragments are evaluated in order, and side effects from earlier
fragments will persist into later fragments:

    {$x = @things; ''}The Lord High Chamberlain has gotten {$x}
    things for me this year.
    { $diff = $x - 17;
      $more = 'more'
      if ($diff == 0) {
        $diff = 'no';
      } elsif ($diff < 0) {
        $more = 'fewer';
      }
      '';
    }
    That is {$diff} {$more} than he gave me last year.

The value of C<$x> set in the first line will persist into the next
fragment that begins on the third line, and the values of C<$diff> and
C<$more> set in the second fragment will persist and be interpolated
into the last line.  The output will look something like this:

    The Lord High Chamberlain has gotten 42
    things for me this year.

    That is 25 more than he gave me last year.

That is all the syntax there is.

=head2 The C<$OUT> variable

There is one special trick you can play in a template.  Here is the
motivation for it:  Suppose you are going to pass an array, C<@items>,
into the template, and you want the template to generate a bulleted
list with a header, like this:

    Here is a list of the things I have got for you since 1907:
      * Ivory
      * Apes
      * Peacocks
      * ...

One way to do it is with a template like this:

    Here is a list of the things I have got for you since 1907:
    { my $blist = '';
      foreach $i (@items) {
          $blist .= qq{  * $i\n};
      }
      $blist;
    }

Here we construct the list in a variable called C<$blist>, which we
return at the end.  This is a little cumbersome.  There is a shortcut.

Inside of templates, there is a special variable called C<$OUT>.
Anything you append to this variable will appear in the output of the
template.  Also, if you use C<$OUT> in a program fragment, the normal
behavior, of replacing the fragment with its return value, is
disabled; instead the fragment is replaced with the value of C<$OUT>.
This means that you can write the template above like this:

    Here is a list of the things I have got for you since 1907:
    { foreach $i (@items) {
        $OUT .= "  * $i\n";
      }
    }

C<$OUT> is reinitialized to the empty string at the start of each
program fragment.  It is private to C<Text::Template>, so 
you can't use a variable named C<$OUT> in your template without
invoking the special behavior.

=head2 General Remarks

All C<Text::Template> functions return C<undef> on failure, and set the
variable C<$Text::Template::ERROR> to contain an explanation of what
went wrong.  For example, if you try to create a template from a file
that does not exist, C<$Text::Template::ERROR> will contain something like:

    Couldn't open file xyz.tmpl: No such file or directory

=head2 C<new>

    $template = Text::Template->new( TYPE => ..., SOURCE => ... );

This creates and returns a new template object.  C<new> returns
C<undef> and sets C<$Text::Template::ERROR> if it can't create the
template object.  C<SOURCE> says where the template source code will
come from.  C<TYPE> says what kind of object the source is.

The most common type of source is a file:

    Text::Template->new( TYPE => 'FILE', SOURCE => $filename );

This reads the template from the specified file.  The filename is
opened with the Perl C<open> command, so it can be a pipe or anything
else that makes sense with C<open>.

The C<TYPE> can also be C<STRING>, in which case the C<SOURCE> should
be a string:

    Text::Template->new( TYPE => 'STRING',
                         SOURCE => "This is the actual template!" );

The C<TYPE> can be C<ARRAY>, in which case the source should be a
reference to an array of strings.  The concatenation of these strings
is the template:

    Text::Template->new( TYPE => 'ARRAY',
                             SOURCE => [ "This is ", "the actual",
                                         " template!",
                                       ]
                       );

The C<TYPE> can be FILEHANDLE, in which case the source should be an
open filehandle (such as you got from the C<FileHandle> or C<IO::*>
packages, or a glob, or a reference to a glob).  In this case
C<Text::Template> will read the text from the filehandle up to
end-of-file, and that text is the template:

    # Read template source code from STDIN:
    Text::Template->new ( TYPE => 'FILEHANDLE', 
                          SOURCE => \*STDIN  );

If you omit the C<TYPE> attribute, it's taken to be C<FILE>.
C<SOURCE> is required.  If you omit it, the program will abort.

The words C<TYPE> and C<SOURCE> can be spelled any of the following ways:

    TYPE     SOURCE
    Type     Source
    type     source
    -TYPE    -SOURCE
    -Type    -Source
    -type    -source

Pick a style you like and stick with it.

=over 4

=item C<DELIMITERS>

You may also add a C<DELIMITERS> option.  If this option is present,
its value should be a reference to an array of two strings.  The first
string is the string that signals the beginning of each program
fragment, and the second string is the string that signals the end of
each program fragment.  See L<"Alternative Delimiters">, below.

=item C<ENCODING>

You may also add a C<ENCODING> option.  If this option is present, and the
C<SOURCE> is a C<FILE>, then the data will be decoded from the given encoding
using the L<Encode> module.  You can use any encoding that L<Encode> recognizes.
E.g.:

    Text::Template->new(
        TYPE     => 'FILE',
        ENCODING => 'UTF-8',
        SOURCE   => 'xyz.tmpl');

=item C<UNTAINT>

If your program is running in taint mode, you may have problems if
your templates are stored in files.  Data read from files is
considered 'untrustworthy', and taint mode will not allow you to
evaluate the Perl code in the file.  (It is afraid that a malicious
person might have tampered with the file.)

In some environments, however, local files are trustworthy.  You can
tell C<Text::Template> that a certain file is trustworthy by supplying
C<UNTAINT =E<gt> 1> in the call to C<new>.  This will tell
C<Text::Template> to disable taint checks on template code that has
come from a file, as long as the filename itself is considered
trustworthy.  It will also disable taint checks on template code that
comes from a filehandle.  When used with C<TYPE =E<gt> 'string'> or C<TYPE
=E<gt> 'array'>, it has no effect.

See L<perlsec> for more complete information about tainting.

Thanks to Steve Palincsar, Gerard Vreeswijk, and Dr. Christoph Baehr
for help with this feature.

=item C<PREPEND>

This option is passed along to the C<fill_in> call unless it is
overridden in the arguments to C<fill_in>.  See L<C<PREPEND> feature
and using C<strict> in templates> below.

=item C<BROKEN>

This option is passed along to the C<fill_in> call unless it is
overridden in the arguments to C<fill_in>.  See L<C<BROKEN>> below.

=back

=head2 C<compile>

    $template->compile()

Loads all the template text from the template's source, parses and
compiles it.  If successful, returns true; otherwise returns false and
sets C<$Text::Template::ERROR>.  If the template is already compiled,
it returns true and does nothing.  

You don't usually need to invoke this function, because C<fill_in>
(see below) compiles the template if it isn't compiled already.

If there is an argument to this function, it must be a reference to an
array containing alternative delimiter strings.  See C<"Alternative
Delimiters">, below.

=head2 C<fill_in>

    $template->fill_in(OPTIONS);

Fills in a template.  Returns the resulting text if successful.
Otherwise, returns C<undef>  and sets C<$Text::Template::ERROR>.

The I<OPTIONS> are a hash, or a list of key-value pairs.  You can
write the key names in any of the six usual styles as above; this
means that where this manual says C<PACKAGE> (for example) you can
actually use any of

    PACKAGE Package package -PACKAGE -Package -package

Pick a style you like and stick with it.  The all-lowercase versions
may yield spurious warnings about

    Ambiguous use of package => resolved to "package"

so you might like to avoid them and use the capitalized versions.

At present, there are eight legal options:  C<PACKAGE>, C<BROKEN>,
C<BROKEN_ARG>, C<FILENAME>, C<SAFE>, C<HASH>, C<OUTPUT>, and C<DELIMITERS>.

=over 4

=item C<PACKAGE>

C<PACKAGE> specifies the name of a package in which the program
fragments should be evaluated.  The default is to use the package from
which C<fill_in> was called.  For example, consider this template:

    The value of the variable x is {$x}.

If you use C<$template-E<gt>fill_in(PACKAGE =E<gt> 'R')> , then the C<$x> in
the template is actually replaced with the value of C<$R::x>.  If you
omit the C<PACKAGE> option, C<$x> will be replaced with the value of
the C<$x> variable in the package that actually called C<fill_in>.

You should almost always use C<PACKAGE>.  If you don't, and your
template makes changes to variables, those changes will be propagated
back into the main program.  Evaluating the template in a private
package helps prevent this.  The template can still modify variables
in your program if it wants to, but it will have to do so explicitly.
See the section at the end on `Security'.

Here's an example of using C<PACKAGE>:

    Your Royal Highness,

    Enclosed please find a list of things I have gotten
    for you since 1907:

    { foreach $item (@items) {
            $item_no++;
        $OUT .= " $item_no. \u$item\n";
      }
    }

    Signed,
    Lord High Chamberlain

We want to pass in an array which will be assigned to the array
C<@items>.  Here's how to do that:

    @items = ('ivory', 'apes', 'peacocks', );
    $template->fill_in();

This is not very safe.  The reason this isn't as safe is that if you
had a variable named C<$item_no> in scope in your program at the point
you called C<fill_in>, its value would be clobbered by the act of
filling out the template.  The problem is the same as if you had
written a subroutine that used those variables in the same way that
the template does.  (C<$OUT> is special in templates and is always
safe.)

One solution to this is to make the C<$item_no> variable private to the
template by declaring it with C<my>.  If the template does this, you
are safe.

But if you use the C<PACKAGE> option, you will probably be safe even
if the template does I<not> declare its variables with C<my>:

    @Q::items = ('ivory', 'apes', 'peacocks', );
    $template->fill_in(PACKAGE => 'Q');

In this case the template will clobber the variable C<$Q::item_no>,
which is not related to the one your program was using.

Templates cannot affect variables in the main program that are
declared with C<my>, unless you give the template references to those
variables.

=item C<HASH>

You may not want to put the template variables into a package.
Packages can be hard to manage:  You can't copy them, for example.
C<HASH> provides an alternative.  

The value for C<HASH> should be a reference to a hash that maps
variable names to values.  For example, 

    $template->fill_in(
        HASH => {
            recipient => "The King",
            items     => ['gold', 'frankincense', 'myrrh'],
            object    => \$self,
        }
    );

will fill out the template and use C<"The King"> as the value of
C<$recipient> and the list of items as the value of C<@items>.  Note
that we pass an array reference, but inside the template it appears as
an array.  In general, anything other than a simple string or number
should be passed by reference.

We also want to pass an object, which is in C<$self>; note that we
pass a reference to the object, C<\$self> instead.  Since we've passed
a reference to a scalar, inside the template the object appears as
C<$object>.  

The full details of how it works are a little involved, so you might
want to skip to the next section.

Suppose the key in the hash is I<key> and the value is I<value>.  

=over 4

=item *

If the I<value> is C<undef>, then any variables named C<$key>,
C<@key>, C<%key>, etc., are undefined.  

=item *

If the I<value> is a string or a number, then C<$key> is set to that
value in the template.

=item *

For anything else, you must pass a reference.

If the I<value> is a reference to an array, then C<@key> is set to
that array.  If the I<value> is a reference to a hash, then C<%key> is
set to that hash.  Similarly if I<value> is any other kind of
reference.  This means that

    var => "foo"

and

    var => \"foo"

have almost exactly the same effect.  (The difference is that in the
former case, the value is copied, and in the latter case it is
aliased.)  

=item *

In particular, if you want the template to get an object or any kind,
you must pass a reference to it:

    $template->fill_in(HASH => { database_handle => \$dbh, ... });

If you do this, the template will have a variable C<$database_handle>
which is the database handle object.  If you leave out the C<\>, the
template will have a hash C<%database_handle>, which exposes the
internal structure of the database handle object; you don't want that.

=back

Normally, the way this works is by allocating a private package,
loading all the variables into the package, and then filling out the
template as if you had specified that package.  A new package is
allocated each time.  However, if you I<also> use the C<PACKAGE>
option, C<Text::Template> loads the variables into the package you
specified, and they stay there after the call returns.  Subsequent
calls to C<fill_in> that use the same package will pick up the values
you loaded in.

If the argument of C<HASH> is a reference to an array instead of a
reference to a hash, then the array should contain a list of hashes
whose contents are loaded into the template package one after the
other.  You can use this feature if you want to combine several sets
of variables.  For example, one set of variables might be the defaults
for a fill-in form, and the second set might be the user inputs, which
override the defaults when they are present:

    $template->fill_in(HASH => [\%defaults, \%user_input]);

You can also use this to set two variables with the same name:

    $template->fill_in(
        HASH => [
            { v => "The King" },
            { v => [1,2,3] }
        ]
    );

This sets C<$v> to C<"The King"> and C<@v> to C<(1,2,3)>.

=item C<BROKEN>

If any of the program fragments fails to compile or aborts for any
reason, and you have set the C<BROKEN> option to a function reference,
C<Text::Template> will invoke the function.  This function is called
the I<C<BROKEN> function>.  The C<BROKEN> function will tell
C<Text::Template> what to do next.  

If the C<BROKEN> function returns C<undef>, C<Text::Template> will
immediately abort processing the template and return the text that it
has accumulated so far.  If your function does this, it should set a
flag that you can examine after C<fill_in> returns so that you can
tell whether there was a premature return or not. 

If the C<BROKEN> function returns any other value, that value will be
interpolated into the template as if that value had been the return
value of the program fragment to begin with.  For example, if the
C<BROKEN> function returns an error string, the error string will be
interpolated into the output of the template in place of the program
fragment that cased the error.

If you don't specify a C<BROKEN> function, C<Text::Template> supplies
a default one that returns something like

    Program fragment delivered error ``Illegal division by 0 at
    template line 37''

(Note that the format of this message has changed slightly since
version 1.31.)  The return value of the C<BROKEN> function is
interpolated into the template at the place the error occurred, so
that this template:

    (3+4)*5 = { 3+4)*5 }

yields this result:

    (3+4)*5 = Program fragment delivered error ``syntax error at template line 1''

If you specify a value for the C<BROKEN> attribute, it should be a
reference to a function that C<fill_in> can call instead of the
default function.

C<fill_in> will pass a hash to the C<broken> function.
The hash will have at least these three members:

=over 4

=item C<text>

The source code of the program fragment that failed

=item C<error>

The text of the error message (C<$@>) generated by eval.

The text has been modified to omit the trailing newline and to include
the name of the template file (if there was one).  The line number
counts from the beginning of the template, not from the beginning of
the failed program fragment.

=item C<lineno>

The line number of the template at which the program fragment began.

=back

There may also be an C<arg> member.  See C<BROKEN_ARG>, below

=item C<BROKEN_ARG>

If you supply the C<BROKEN_ARG> option to C<fill_in>, the value of the
option is passed to the C<BROKEN> function whenever it is called.  The
default C<BROKEN> function ignores the C<BROKEN_ARG>, but you can
write a custom C<BROKEN> function that uses the C<BROKEN_ARG> to get
more information about what went wrong. 

The C<BROKEN> function could also use the C<BROKEN_ARG> as a reference
to store an error message or some other information that it wants to
communicate back to the caller.  For example:

    $error = '';

    sub my_broken {
       my %args = @_;
       my $err_ref = $args{arg};
       ...
       $$err_ref = "Some error message";
       return undef;
    }

    $template->fill_in(
        BROKEN     => \&my_broken,
        BROKEN_ARG => \$error
    );

    if ($error) {
      die "It didn't work: $error";
    }

If one of the program fragments in the template fails, it will call
the C<BROKEN> function, C<my_broken>, and pass it the C<BROKEN_ARG>,
which is a reference to C<$error>.  C<my_broken> can store an error
message into C<$error> this way.  Then the function that called
C<fill_in> can see if C<my_broken> has left an error message for it
to find, and proceed accordingly.

=item C<FILENAME>

If you give C<fill_in> a C<FILENAME> option, then this is the file name that
you loaded the template source from.  This only affects the error message that
is given for template errors.  If you loaded the template from C<foo.txt> for
example, and pass C<foo.txt> as the C<FILENAME> parameter, errors will look
like C<... at foo.txt line N> rather than C<... at template line N>. 

Note that this does NOT have anything to do with loading a template from the
given filename.  See C<fill_in_file()> for that.

For example:

 my $template = Text::Template->new(
     TYPE   => 'string',
     SOURCE => 'The value is {1/0}');

 $template->fill_in(FILENAME => 'foo.txt') or die $Text::Template::ERROR;

will die with an error that contains

 Illegal division by zero at at foo.txt line 1

=item C<SAFE>

If you give C<fill_in> a C<SAFE> option, its value should be a safe
compartment object from the C<Safe> package.  All evaluation of
program fragments will be performed in this compartment.  See L<Safe>
for full details about such compartments and how to restrict the
operations that can be performed in them.

If you use the C<PACKAGE> option with C<SAFE>, the package you specify
will be placed into the safe compartment and evaluation will take
place in that package as usual.  

If not, C<SAFE> operation is a little different from the default.
Usually, if you don't specify a package, evaluation of program
fragments occurs in the package from which the template was invoked.
But in C<SAFE> mode the evaluation occurs inside the safe compartment
and cannot affect the calling package.  Normally, if you use C<HASH>
without C<PACKAGE>, the hash variables are imported into a private,
one-use-only package.  But if you use C<HASH> and C<SAFE> together
without C<PACKAGE>, the hash variables will just be loaded into the
root namespace of the C<Safe> compartment.

=item C<OUTPUT>

If your template is going to generate a lot of text that you are just
going to print out again anyway,  you can save memory by having
C<Text::Template> print out the text as it is generated instead of
making it into a big string and returning the string.  If you supply
the C<OUTPUT> option to C<fill_in>, the value should be a filehandle.
The generated text will be printed to this filehandle as it is
constructed.  For example:

    $template->fill_in(OUTPUT => \*STDOUT, ...);

fills in the C<$template> as usual, but the results are immediately
printed to STDOUT.  This may result in the output appearing more
quickly than it would have otherwise.

If you use C<OUTPUT>, the return value from C<fill_in> is still true on
success and false on failure, but the complete text is not returned to
the caller.

=item C<PREPEND>

You can have some Perl code prepended automatically to the beginning
of every program fragment.  See L<C<PREPEND> feature and using
C<strict> in templates> below.

=item C<DELIMITERS>

If this option is present, its value should be a reference to a list
of two strings.  The first string is the string that signals the
beginning of each program fragment, and the second string is the
string that signals the end of each program fragment.  See
L<"Alternative Delimiters">, below.  

If you specify C<DELIMITERS> in the call to C<fill_in>, they override
any delimiters you set when you created the template object with
C<new>. 

=back

=head1 Convenience Functions

=head2 C<fill_this_in>

The basic way to fill in a template is to create a template object and
then call C<fill_in> on it.   This is useful if you want to fill in
the same template more than once.

In some programs, this can be cumbersome.  C<fill_this_in> accepts a
string, which contains the template, and a list of options, which are
passed to C<fill_in> as above.  It constructs the template object for
you, fills it in as specified, and returns the results.  It returns
C<undef> and sets C<$Text::Template::ERROR> if it couldn't generate
any results.

An example:

    $Q::name = 'Donald';
    $Q::amount = 141.61;
    $Q::part = 'hyoid bone';

    $text = Text::Template->fill_this_in( <<'EOM', PACKAGE => Q);
    Dear {$name},
    You owe me \\${sprintf('%.2f', $amount)}.
    Pay or I will break your {$part}.
        Love,
        Grand Vizopteryx of Irkutsk.
    EOM

Notice how we included the template in-line in the program by using a
`here document' with the C<E<lt>E<lt>> notation.

C<fill_this_in> is a deprecated feature.  It is only here for
backwards compatibility, and may be removed in some far-future version
in C<Text::Template>.  You should use C<fill_in_string> instead.  It
is described in the next section.

=head2 C<fill_in_string>

It is stupid that C<fill_this_in> is a class method.  It should have
been just an imported function, so that you could omit the
C<Text::Template-E<gt>> in the example above.  But I made the mistake
four years ago and it is too late to change it.

C<fill_in_string> is exactly like C<fill_this_in> except that it is
not a method and you can omit the C<Text::Template-E<gt>> and just say

    print fill_in_string(<<'EOM', ...);
    Dear {$name},
      ...
    EOM

To use C<fill_in_string>, you need to say

    use Text::Template 'fill_in_string';

at the top of your program.   You should probably use
C<fill_in_string> instead of C<fill_this_in>.

=head2 C<fill_in_file>

If you import C<fill_in_file>, you can say

    $text = fill_in_file(filename, ...);

The C<...> are passed to C<fill_in> as above.  The filename is the
name of the file that contains the template you want to fill in.  It
returns the result text. or C<undef>, as usual.

If you are going to fill in the same file more than once in the same
program you should use the longer C<new> / C<fill_in> sequence instead.
It will be a lot faster because it only has to read and parse the file
once.

=head2 Including files into templates

People always ask for this.  ``Why don't you have an include
function?'' they want to know.  The short answer is this is Perl, and
Perl already has an include function.  If you want it, you can just put

    {qx{cat filename}}

into your template.  VoilE<agrave>.

If you don't want to use C<cat>, you can write a little four-line
function that opens a file and dumps out its contents, and call it
from the template.  I wrote one for you.  In the template, you can say

    {Text::Template::_load_text(filename)}

If that is too verbose, here is a trick.  Suppose the template package
that you are going to be mentioning in the C<fill_in> call is package
C<Q>.  Then in the main program, write

    *Q::include = \&Text::Template::_load_text;

This imports the C<_load_text> function into package C<Q> with the
name C<include>.  From then on, any template that you fill in with
package C<Q> can say

    {include(filename)}

to insert the text from the named file at that point.  If you are
using the C<HASH> option instead, just put C<include =E<gt>
\&Text::Template::_load_text> into the hash instead of importing it
explicitly.

Suppose you don't want to insert a plain text file, but rather you
want to include one template within another?  Just use C<fill_in_file>
in the template itself:

    {Text::Template::fill_in_file(filename)}

You can do the same importing trick if this is too much to type.

=head1 Miscellaneous

=head2 C<my> variables

People are frequently surprised when this doesn't work:

    my $recipient = 'The King';
    my $text = fill_in_file('formletter.tmpl');

The text C<The King> doesn't get into the form letter.  Why not?
Because C<$recipient> is a C<my> variable, and the whole point of
C<my> variables is that they're private and inaccessible except in the
scope in which they're declared.  The template is not part of that
scope, so the template can't see C<$recipient>.  

If that's not the behavior you want, don't use C<my>.  C<my> means a
private variable, and in this case you don't want the variable to be
private.  Put the variables into package variables in some other
package, and use the C<PACKAGE> option to C<fill_in>:

    $Q::recipient = $recipient;
    my $text = fill_in_file('formletter.tmpl', PACKAGE => 'Q');

or pass the names and values in a hash with the C<HASH> option:

    my $text = fill_in_file('formletter.tmpl', HASH => { recipient => $recipient });

=head2 Security Matters

All variables are evaluated in the package you specify with the
C<PACKAGE> option of C<fill_in>.  if you use this option, and if your
templates don't do anything egregiously stupid, you won't have to
worry that evaluation of the little programs will creep out into the
rest of your program and wreck something.

Nevertheless, there's really no way (except with C<Safe>) to protect
against a template that says

    { $Important::Secret::Security::Enable = 0;
      # Disable security checks in this program
    }

or

    { $/ = "ho ho ho";   # Sabotage future uses of <FH>.
      # $/ is always a global variable
    }

or even

    { system("rm -rf /") }

so B<don't> go filling in templates unless you're sure you know what's
in them.  If you're worried, or you can't trust the person who wrote
the template, use the C<SAFE> option.

A final warning: program fragments run a small risk of accidentally
clobbering local variables in the C<fill_in> function itself.  These
variables all have names that begin with C<$fi_>, so if you stay away
from those names you'll be safe.  (Of course, if you're a real wizard
you can tamper with them deliberately for exciting effects; this is
actually how C<$OUT> works.)  I can fix this, but it will make the
package slower to do it, so I would prefer not to.  If you are worried
about this, send me mail and I will show you what to do about it.

=head2 Alternative Delimiters

Lorenzo Valdettaro pointed out that if you are using C<Text::Template>
to generate TeX output, the choice of braces as the program fragment
delimiters makes you suffer suffer suffer.  Starting in version 1.20,
you can change the choice of delimiters to something other than curly
braces.

In either the C<new()> call or the C<fill_in()> call, you can specify
an alternative set of delimiters with the C<DELIMITERS> option.  For
example, if you would like code fragments to be delimited by C<[@-->
and C<--@]> instead of C<{> and C<}>, use

    ... DELIMITERS => [ '[@--', '--@]' ], ...

Note that these delimiters are I<literal strings>, not regexes.  (I
tried for regexes, but it complicates the lexical analysis too much.)
Note also that C<DELIMITERS> disables the special meaning of the
backslash, so if you want to include the delimiters in the literal
text of your template file, you are out of luck---it is up to you to
choose delimiters that do not conflict with what you are doing.  The
delimiter strings may still appear inside of program fragments as long
as they nest properly.  This means that if for some reason you
absolutely must have a program fragment that mentions one of the
delimiters, like this:

    [@--
        print "Oh no, a delimiter: --@]\n"
    --@]

you may be able to make it work by doing this instead:

    [@--
        # Fake matching delimiter in a comment: [@--
        print "Oh no, a delimiter: --@]\n"
    --@]

It may be safer to choose delimiters that begin with a newline
character.  

Because the parsing of templates is simplified by the absence of
backslash escapes, using alternative C<DELIMITERS> may speed up the
parsing process by 20-25%.  This shows that my original choice of C<{>
and C<}> was very bad. 

=head2 C<PREPEND> feature and using C<strict> in templates

Suppose you would like to use C<strict> in your templates to detect
undeclared variables and the like.  But each code fragment is a
separate lexical scope, so you have to turn on C<strict> at the top of
each and every code fragment:

    { use strict;
      use vars '$foo';
      $foo = 14;
      ...
    }

    ...

    { # we forgot to put `use strict' here
      my $result = $boo + 12;    # $boo is misspelled and should be $foo
      # No error is raised on `$boo'
    }

Because we didn't put C<use strict> at the top of the second fragment,
it was only active in the first fragment, and we didn't get any
C<strict> checking in the second fragment.  Then we misspelled C<$foo>
and the error wasn't caught.  

C<Text::Template> version 1.22 and higher has a new feature to make
this easier.  You can specify that any text at all be automatically
added to the beginning of each program fragment.  

When you make a call to C<fill_in>, you can specify a

    PREPEND => 'some perl statements here'

option; the statements will be prepended to each program fragment for
that one call only.  Suppose that the C<fill_in> call included a

    PREPEND => 'use strict;'

option, and that the template looked like this:

    { use vars '$foo';
      $foo = 14;
      ...
    }

    ...

    { my $result = $boo + 12;    # $boo is misspelled and should be $foo
      ...
    }

The code in the second fragment would fail, because C<$boo> has not
been declared.  C<use strict> was implied, even though you did not
write it explicitly, because the C<PREPEND> option added it for you
automatically.

There are three other ways to do this.  At the time you create the
template object with C<new>, you can also supply a C<PREPEND> option,
in which case the statements will be prepended each time you fill in
that template.  If the C<fill_in> call has its own C<PREPEND> option,
this overrides the one specified at the time you created the
template.  Finally, you can make the class method call

    Text::Template->always_prepend('perl statements');

If you do this, then call calls to C<fill_in> for I<any> template will
attach the perl statements to the beginning of each program fragment,
except where overridden by C<PREPEND> options to C<new> or C<fill_in>.

An alternative to adding "use strict;" to the PREPEND option, you can
pass STRICT => 1 to fill_in when also passing the HASH option.

Suppose that the C<fill_in> call included both

    HASH   => {$foo => ''} and
    STRICT => 1

options, and that the template looked like this:

    {
      $foo = 14;
      ...
    }

    ...

    { my $result = $boo + 12;    # $boo is misspelled and should be $foo
      ...
    }

The code in the second fragment would fail, because C<$boo> has not
been declared. C<use strict> was implied, even though you did not
write it explicitly, because the C<STRICT> option added it for you
automatically. Any variable referenced in the template that is not in the
C<HASH> option will be an error.

=head2 Prepending in Derived Classes

This section is technical, and you should skip it on the first few
readings. 

Normally there are three places that prepended text could come from.
It could come from the C<PREPEND> option in the C<fill_in> call, from
the C<PREPEND> option in the C<new> call that created the template
object, or from the argument of the C<always_prepend> call.
C<Text::Template> looks for these three things in order and takes the
first one that it finds.

In a subclass of C<Text::Template>, this last possibility is
ambiguous.  Suppose C<S> is a subclass of C<Text::Template>.  Should 

    Text::Template->always_prepend(...);

affect objects in class C<Derived>?  The answer is that you can have it
either way.  

The C<always_prepend> value for C<Text::Template> is normally stored
in  a hash variable named C<%GLOBAL_PREPEND> under the key
C<Text::Template>.  When C<Text::Template> looks to see what text to
prepend, it first looks in the template object itself, and if not, it
looks in C<$GLOBAL_PREPEND{I<class>}> where I<class> is the class to
which the template object belongs.  If it doesn't find any value, it
looks in C<$GLOBAL_PREPEND{'Text::Template'}>.  This means that
objects in class C<Derived> I<will> be affected by

    Text::Template->always_prepend(...);

I<unless> there is also a call to

    Derived->always_prepend(...);

So when you're designing your derived class, you can arrange to have
your objects ignore C<Text::Template::always_prepend> calls by simply
putting C<Derived-E<gt>always_prepend('')> at the top of your module.

Of course, there is also a final escape hatch: Templates support a
C<prepend_text> that is used to look up the appropriate text to be
prepended at C<fill_in> time.  Your derived class can override this
method to get an arbitrary effect.

=head2 JavaScript

Jennifer D. St Clair asks:

    > Most of my pages contain JavaScript and Stylesheets.
    > How do I change the template identifier?

Jennifer is worried about the braces in the JavaScript being taken as
the delimiters of the Perl program fragments.  Of course, disaster
will ensue when perl tries to evaluate these as if they were Perl
programs.  The best choice is to find some unambiguous delimiter
strings that you can use in your template instead of curly braces, and
then use the C<DELIMITERS> option.  However, if you can't do this for
some reason, there are  two easy workarounds:

1. You can put C<\> in front of C<{>, C<}>, or C<\> to remove its
special meaning.  So, for example, instead of

    if (br== "n3") { 
        // etc.
    }

you can put

    if (br== "n3") \{
        // etc.
    \}

and it'll come out of the template engine the way you want.

But here is another method that is probably better.  To see how it
works, first consider what happens if you put this into a template:

    { 'foo' }

Since it's in braces, it gets evaluated, and obviously, this is going
to turn into

    foo

So now here's the trick: In Perl, C<q{...}> is the same as C<'...'>.
So if we wrote

    {q{foo}}

it would turn into 

    foo

So for your JavaScript, just write

    {q{if (br== "n3") {
       // etc.
       }}
    }

and it'll come out as

    if (br== "n3") {
        // etc.
    }

which is what you want.

head2 Shut Up!

People sometimes try to put an initialization section at the top of
their templates, like this:

    { ...
        $var = 17;
    }

Then they complain because there is a C<17> at the top of the output
that they didn't want to have there.  

Remember that a program fragment is replaced with its own return
value, and that in Perl the return value of a code block is the value
of the last expression that was evaluated, which in this case is 17.
If it didn't do that, you wouldn't be able to write C<{$recipient}>
and have the recipient filled in.

To prevent the 17 from appearing in the output is very simple:

    { ...
        $var = 17;
        '';
    }

Now the last expression evaluated yields the empty string, which is
invisible.  If you don't like the way this looks, use

    { ...
        $var = 17;
        ($SILENTLY);
    }

instead.  Presumably, C<$SILENTLY> has no value, so nothing will be
interpolated.  This is what is known as a `trick'.

=head2 Compatibility

Every effort has been made to make this module compatible with older
versions.  The only known exceptions follow:

The output format of the default C<BROKEN> subroutine has changed
twice, most recently between versions 1.31 and 1.40.

Starting in version 1.10, the C<$OUT> variable is arrogated for a
special meaning.  If you had templates before version 1.10 that
happened to use a variable named C<$OUT>, you will have to change them
to use some other variable or all sorts of strangeness will result.

Between versions 0.1b and 1.00 the behavior of the \ metacharacter
changed.  In 0.1b, \\ was special everywhere, and the template
processor always replaced it with a single backslash before passing
the code to Perl for evaluation.  The rule now is more complicated but
probably more convenient.  See the section on backslash processing,
below, for a full discussion.

=head2 Backslash Processing

In C<Text::Template> beta versions, the backslash was special whenever
it appeared before a brace or another backslash.  That meant that
while C<{"\n"}> did indeed generate a newline, C<{"\\"}> did not
generate a backslash, because the code passed to Perl for evaluation
was C<"\"> which is a syntax error.  If you wanted a backslash, you
would have had to write C<{"\\\\"}>.

In C<Text::Template> versions 1.00 through 1.10, there was a bug:
Backslash was special everywhere.  In these versions, C<{"\n"}>
generated the letter C<n>.

The bug has been corrected in version 1.11, but I did not go back to
exactly the old rule, because I did not like the idea of having to
write C<{"\\\\"}> to get one backslash.  The rule is now more
complicated to remember, but probably easier to use.  The rule is now:
Backslashes are always passed to Perl unchanged I<unless> they occur
as part of a sequence like C<\\\\\\{> or C<\\\\\\}>.  In these
contexts, they are special; C<\\> is replaced with C<\>, and C<\{> and
C<\}> signal a literal brace. 

Examples:

    \{ foo \}

is I<not> evaluated, because the C<\> before the braces signals that
they should be taken literally.  The result in the output looks like this: 

    { foo }

This is a syntax error:

    { "foo}" }

because C<Text::Template> thinks that the code ends at the first C<}>,
and then gets upset when it sees the second one.  To make this work
correctly, use

    { "foo\}" }

This passes C<"foo}"> to Perl for evaluation.  Note there's no C<\> in
the evaluated code.  If you really want a C<\> in the evaluated code,
use

    { "foo\\\}" }

This passes C<"foo\}"> to Perl for evaluation.

Starting with C<Text::Template> version 1.20, backslash processing is
disabled if you use the C<DELIMITERS> option to specify alternative
delimiter strings.

=head2 A short note about C<$Text::Template::ERROR>

In the past some people have fretted about `violating the package
boundary' by examining a variable inside the C<Text::Template>
package.  Don't feel this way.  C<$Text::Template::ERROR> is part of
the published, official interface to this package.  It is perfectly OK
to inspect this variable.  The interface is not going to change.

If it really, really bothers you, you can import a function called
C<TTerror> that returns the current value of the C<$ERROR> variable.
So you can say:

    use Text::Template 'TTerror';

    my $template = Text::Template->new(SOURCE => $filename);
    unless ($template) {
        my $err = TTerror;
        die "Couldn't make template: $err; aborting";
    }

I don't see what benefit this has over just doing this:

    use Text::Template;

    my $template = Text::Template->new(SOURCE => $filename)
        or die "Couldn't make template: $Text::Template::ERROR; aborting";

But if it makes you happy to do it that way, go ahead.

=head2 Sticky Widgets in Template Files

The C<CGI> module provides functions for `sticky widgets', which are
form input controls that retain their values from one page to the
next.   Sometimes people want to know how to include these widgets
into their template output.

It's totally straightforward.  Just call the C<CGI> functions from
inside the template:

    { $q->checkbox_group(NAME      => 'toppings',
                         LINEBREAK => true,
                         COLUMNS   => 3,
                         VALUES    => \@toppings,
                        );
    }

=head2 Automatic preprocessing of program fragments

It may be useful to preprocess the program fragments before they are
evaluated.  See C<Text::Template::Preprocess> for more details.

=head2 Automatic postprocessing of template hunks

It may be useful to process hunks of output before they are appended to
the result text.  For this, subclass and replace the C<append_text_to_result>
method.  It is passed a list of pairs with these entries:

  handle - a filehandle to which to print the desired output
  out    - a ref to a string to which to append, to use if handle is not given
  text   - the text that will be appended
  type   - where the text came from: TEXT for literal text, PROG for code

=head1 HISTORY

Originally written by Mark Jason Dominus, Plover Systems (versions 0.01 - 1.46)

Maintainership transferred to Michael Schout E<lt>mschout@cpan.orgE<gt> in version
1.47

=head1 THANKS

Many thanks to the following people for offering support,
encouragement, advice, bug reports, and all the other good stuff.  

=over 4

=item *

Andrew G Wood

=item *

Andy Wardley

=item *

António Aragão

=item *

Archie Warnock

=item *

Bek Oberin

=item *

Bob Dougherty

=item *

Brian C. Shensky

=item *

Chris Nandor

=item *

Chris Wesley

=item *

Chris.Brezil

=item *

Daini Xie

=item *

Dan Franklin

=item *

Daniel LaLiberte

=item *

David H. Adler

=item *

David Marshall

=item *

Dennis Taylor

=item *

Donald L. Greer Jr.

=item *

Dr. Frank Bucolo

=item *

Fred Steinberg

=item *

Gene Damon

=item *

Hans Persson

=item *

Hans Stoop

=item *

Itamar Almeida de Carvalho

=item *

James H. Thompson

=item *

James Mastros

=item *

Jarko Hietaniemi

=item *

Jason Moore

=item *

Jennifer D. St Clair

=item *

Joel Appelbaum

=item *

Joel Meulenberg

=item *

Jonathan Roy

=item *

Joseph Cheek

=item *

Juan E. Camacho

=item *

Kevin Atteson

=item *

Kevin Madsen

=item *

Klaus Arnhold

=item *

Larry Virden

=item *

Lieven Tomme

=item *

Lorenzo Valdettaro

=item *

Marek Grac

=item *

Matt Womer

=item *

Matt X. Hunter

=item *

Michael G Schwern

=item *

Michael J. Suzio

=item *

Michaely Yeung

=item *

Michelangelo Grigni

=item *

Mike Brodhead

=item *

Niklas Skoglund

=item *

Randal L. Schwartz

=item *

Reuven M. Lerner

=item *

Robert M. Ioffe

=item *

Ron Pero

=item *

San Deng

=item *

Sean Roehnelt

=item *

Sergey Myasnikov

=item *

Shabbir J. Safdar

=item *

Shad Todd

=item *

Steve Palincsar

=item *

Tim Bunce

=item *

Todd A. Green

=item *

Tom Brown

=item *

Tom Henry

=item *

Tom Snee

=item *

Trip Lilley

=item *

Uwe Schneider

=item *

Val Luck

=item *

Yannis Livassof

=item *

Yonat Sharon

=item *

Zac Hansen

=item *

gary at dls.net

=back

Special thanks to:

=over 2

=item Jonathan Roy 

for telling me how to do the C<Safe> support (I spent two years
worrying about it, and then Jonathan pointed out that it was trivial.)

=item Ranjit Bhatnagar 

for demanding less verbose fragments like they have in ASP, for
helping me figure out the Right Thing, and, especially, for talking me
out of adding any new syntax.  These discussions resulted in the
C<$OUT> feature.

=back

=head2 Bugs and Caveats

C<my> variables in C<fill_in> are still susceptible to being clobbered
by template evaluation.  They all begin with C<fi_>, so avoid those
names in your templates.

The line number information will be wrong if the template's lines are
not terminated by C<"\n">.  You should let me know if this is a
problem.  If you do, I will fix it.

The C<$OUT> variable has a special meaning in templates, so you cannot
use it as if it were a regular variable.

There are not quite enough tests in the test suite.

=head1 SOURCE

The development version is on github at L<https://https://github.com/mschout/perl-text-template>
and may be cloned from L<git://https://github.com/mschout/perl-text-template.git>

=head1 BUGS

Please report any bugs or feature requests on the bugtracker website
L<https://github.com/mschout/perl-text-template/issues>

When submitting a bug or request, please include a test-file or a
patch to an existing test-file that illustrates the bug or desired
feature.

=head1 AUTHOR

Michael Schout <mschout@cpan.org>

=head1 COPYRIGHT AND LICENSE

This software is copyright (c) 2013 by Mark Jason Dominus <mjd@cpan.org>.

This is free software; you can redistribute it and/or modify it under
the same terms as the Perl 5 programming language system itself.

=cut

#! /usr/bin/env perl
#
# C source compressor.  This:
#
# - merges continuation lines
# - removes comments (not in strings)
# - removes empty lines (not in strings)

use strict;
use warnings;

my $debug = defined $ENV{DEBUG};
my $lang = shift @ARGV;

# Slurp the file
$/ = undef;
$_ = <>;

if ($lang eq 'C') {
    # Merge continuation lines
    s{\\\n}{}g;

    # Regexp for things that should be preserved
    my $preserved =
        qr{
              (?:
                  "                 # String start
                  (?: \\. | [^\"])* # Any character, including escaped ones
                  "                 # String end
              )

          |                         # OR

              (?:
                  '                 # Character start (multi-chars supported)
                  (?: \\. | [^\'])+ # Any character, including escaped ones
                  '                 # String end
              )
        }x;

    # Remove comments while preserving strings
    s{
         (?|                        # All things preserved end up in $1

             /\*                    # C comment start
             .*?                    # Contents up until
             \*/                    # C comment end

         |                          # OR

             (                      # Grouping for the replacement
                 $preserved
             )

         )
    }{
        if ($debug) {
            print STDERR "DEBUG: '$&' => '$1'\n" if defined $1;
            print STDERR "DEBUG: '$&' removed\n" unless defined $1;
        }
        defined $1 ? $1 : ""
    }gsxe;

    # Remove empty lines
    s{
         (?|                        # All things preserved end up in $1

             (^|\n)(?:\s*(?:\n|$))+ # Empty lines, preserve one newline

         |                          # OR

             (                      # Grouping for the replacement
                 $preserved
             )

         )
    }{$1}gsx;

    # Remove extra spaces
    s{
         (?|                        # All things preserved end up in $1

             \h+                    # Horizontal spaces replaced with one

         |                          # OR

             (                      # Grouping for the replacement
                 $preserved
             )

         )
    }{
        if ($debug) {
            print STDERR "DEBUG: '$&' => '$1'\n" if defined $1;
            print STDERR "DEBUG: '$&' => ' '\n" unless defined $1;
        }
        defined $1 ? $1 : " "
    }gsxe;

    # Clean up spaces at start and end of lines
    s/^ //mg;
    s/ $//mg;
} elsif ($lang eq 'S') {
    # Because we use C++ style comments in our .S files, all we can do
    # is to drop them
    s{
         ^([^\n]*?)//[^\n]*?$   # Any line with a // comment
    }{
        if ($debug) {
            print STDERR "DEBUG: '$&' => '$1'\n" if defined $1;
            print STDERR "DEBUG: '$&' removed\n" unless defined $1;
        }
        defined $1 ? $1 : ""
    }mgsxe;

    # Drop all empty lines
    s{
         (^|\n)(?:\s*(?:\n|$))+ # Empty lines, preserve one newline
    }{$1}gsx;
} elsif ($lang eq 'perl') {
    # Merge continuation lines
    s{\\\n}{}g;

    # Regexp for things that should be preserved
    my $preserved =
        qr{
              (?:
                  <<["']?(\w+)["']? # HERE document start
                  .*?               # Its contents
                  ^\g{-1}$
              )
          |
              (?:
                  "                 # Double quoted string start
                  (?: \\. | [^\"])* # Any character, including escaped ones
                  "                 # Double quoted string end
              )

          |                         # OR

              (?:
                  '                 # Single quoted string start
                  [^\']*            # Any character
                  '                 # Single quoted string end
              )
        }msx;

    # Remove comments while preserving strings
    s{
         (?|                        # All things preserved end up in $1

             \#.*?(\n|$)            # Perl comments

         |                          # OR

             (                      # Grouping for the replacement
                 $preserved
             )

         )
    }{
        if ($debug) {
            print STDERR "DEBUG: '$&' => '$1'\n" if defined $1;
            print STDERR "DEBUG: '$&' removed\n" unless defined $1;
        }
        defined $1 ? $1 : ""
    }gsxe;

    # Remove empty lines
    s{
         (?|                        # All things preserved end up in $1

             (^|\n)(?:\s*(?:\n|$))+ # Empty lines, preserve one newline

         |                          # OR

             (                      # Grouping for the replacement
                 $preserved
             )

         )
    }{$1}gsx;
}

print;

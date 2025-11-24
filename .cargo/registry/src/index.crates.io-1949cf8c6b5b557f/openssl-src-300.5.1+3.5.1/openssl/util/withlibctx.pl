#! /usr/bin/env perl

use strict;
use warnings;
use File::Temp qw/tempfile/;

my $topdir = shift;

processallfiles($topdir);
print "Success\n";

sub processallfiles {
    my $dir = shift;
    my @files = glob "$dir/*.c $dir/*.h $dir/*.h.in $dir/*.pod *dir/*.pod.in";

    open (my $STDOUT_ORIG, '>&', STDOUT);

    foreach my $file (@files) {
        my ($tmpfh, $tmpfile) = tempfile();

        print "Processing $file\n";
        open(STDOUT, '>>', $tmpfile);
        open(INFILE, $file);
        processfile(\*INFILE);
        close(STDOUT);
        rename($tmpfile, $file);
        unlink($tmpfile);
        # restore STDOUT
        open (STDOUT, '>&', $STDOUT_ORIG);
    }

    #Recurse through subdirs
    opendir my $dh, $dir or die "Cannot open directory";

    while (defined(my $subdir = readdir $dh)) {
        next unless -d "$dir/$subdir";
        next if (rindex $subdir, ".", 0) == 0;
        processallfiles("$dir/$subdir");
    }
    closedir $dh;
}

sub processfile {
    my $fh = shift;
    my $multiline = 0;
    my @params;
    my $indent;
    my $paramstr = "";

    foreach my $line (<$fh>) {
        chomp($line);
        if (!$multiline) {
            if ($line =~ /^(.+)_with_libctx\((.*[^\\])$/) {
                my $preline = $1;
                my $postline = $2;
                #Strip trailing whitespace
                $postline =~ s/\s+$//;
                print $preline.'_ex(';
                my @rets = extracttoclose($postline);
                if (@rets) {
                    print "$postline\n";
                    $multiline = 0;
                } else {
                    $multiline = 1;
                    $paramstr = $postline;
                    $indent = (length $preline) + (length '_ex(');
                }
            } else {
                #Any other reference to _with_libctx we just replace
                $line =~ s/_with_libctx/_ex/g;
                print $line."\n";
            }
        } else {
            #Strip leading whitespace
            $line =~ s/^\s+//;
            #Strip trailing whitespace
            $line =~ s/\s+$//;
            my @rets = extracttoclose($paramstr.$line);
            if (@rets) {
                my $pre = shift @rets;
                my $post = shift @rets;
                @params = split(",", $pre);
                my @params = grep(s/^\s*|\s*$//g, @params);
                formatparams($indent, @params);
                print ')'.$post."\n";
                $multiline = 0;
            } else {
                $paramstr .= $line;
            }
        }
    }

    die "End of multiline not found" if $multiline;
}

sub formatparams {
    my $indent = shift;
    my @params = @_;

    if (@params) {
        my $param = shift @params;
        my $lensofar += $indent + (length $param) + 1;

        print "$param";
        print "," if @params;

        while (@params) {
            my $param = shift @params;

            if (($lensofar + (length $param) + 2) > 80) {
                print "\n".(" " x $indent);
                print $param;
                $lensofar = $indent + (length $param) + 1;
            } else {
                print ' '.$param;
                $lensofar += (length $param) + 2;
            }
            print "," if @params;
        }
    }
}

sub extracttoclose {
    my $inline = shift;
    my $outline = "";

    while ($inline =~ /^([^\)]*?)\((.*)$/) {
        my @rets = extracttoclose($2);
        if (!@rets) {
            return ();
        }
        my $inside = shift @rets;
        my $post = shift @rets;
        $outline .= $1.'('.$inside.')';
        $inline = $post;
    }
    if ($inline =~ /^(.*?)\)(.*)$/) {
        return ($outline.$1, $2);
    }
    return ();
}

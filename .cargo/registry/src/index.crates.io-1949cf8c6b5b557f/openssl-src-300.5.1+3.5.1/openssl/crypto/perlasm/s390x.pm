#!/usr/bin/env perl
# Copyright 2018-2020 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

# Copyright IBM Corp. 2018-2019
# Author: Patrick Steuer <patrick.steuer@de.ibm.com>

package perlasm::s390x;

use strict;
use warnings;
use bigint;
use Carp qw(confess);
use Exporter qw(import);

our @EXPORT=qw(PERLASM_BEGIN PERLASM_END);
our @EXPORT_OK=qw(AUTOLOAD LABEL INCLUDE stfle stck);
our %EXPORT_TAGS=(
	# store-clock-fast facility
	SCF => [qw(stckf)],
	# general-instruction-extension facility
	GE => [qw(risbg)],
	# extended-immediate facility
	EI => [qw(clfi clgfi lt)],
	# miscellaneous-instruction-extensions facility 1
	MI1 => [qw(risbgn)],
	# message-security assist
	MSA => [qw(kmac km kmc kimd klmd)],
	# message-security-assist extension 4
	MSA4 => [qw(kmf kmo pcc kmctr)],
	# message-security-assist extension 5
	MSA5 => [qw(ppno prno)],
	# message-security-assist extension 8
	MSA8 => [qw(kma)],
	# message-security-assist extension 9
	MSA9 => [qw(kdsa)],
	# vector facility
	VX => [qw(vgef vgeg vgbm vzero vone vgm vgmb vgmh vgmf vgmg
	    vl vlr vlrep vlrepb vlreph vlrepf vlrepg vleb vleh vlef vleg vleib
	    vleih vleif vleig vlgv vlgvb vlgvh vlgvf vlgvg vllez vllezb vllezh
	    vllezf vllezg vlm vlbb vlvg vlvgb vlvgh vlvgf vlvgg vlvgp
	    vll vmrh vmrhb vmrhh vmrhf vmrhg vmrl vmrlb vmrlh vmrlf vmrlg vpk
	    vpkh vpkf vpkg vpks vpksh vpksf vpksg vpkshs vpksfs vpksgs vpkls
	    vpklsh vpklsf vpklsg vpklshs vpklsfs vpklsgs vperm vpdi vrep vrepb
	    vreph vrepf vrepg vrepi vrepib vrepih vrepif vrepig vscef vsceg
	    vsel vseg vsegb vsegh vsegf vst vsteb vsteh vstef vsteg vstm vstl
	    vuph vuphb vuphh vuphf vuplh vuplhb vuplhh vuplhf vupl vuplb vuplhw
	    vuplf vupll vupllb vupllh vupllf va vab vah vaf vag vaq vacc vaccb
	    vacch vaccf vaccg vaccq vac vacq vaccc vacccq vn vnc vavg vavgb
	    vavgh vavgf vavgg vavgl vavglb vavglh vavglf vavglg vcksm vec_ vecb
	    vech vecf vecg vecl veclb veclh veclf veclg vceq vceqb vceqh vceqf
	    vceqg vceqbs vceqhs vceqfs vceqgs vch vchb vchh vchf vchg vchbs
	    vchhs vchfs vchgs vchl vchlb vchlh vchlf vchlg vchlbs vchlhs vchlfs
	    vchlgs vclz vclzb vclzh vclzf vclzg vctz vctzb vctzh vctzf vctzg
	    vx vgfm vgfmb vgfmh vgfmf vgfmg vgfma vgfmab vgfmah vgfmaf vgfmag
	    vlc vlcb vlch vlcf vlcg vlp vlpb vlph vlpf vlpg vmx vmxb vmxh vmxf
	    vmxg vmxl vmxlb vmxlh vmxlf vmxlg vmn vmnb vmnh vmnf vmng vmnl
	    vmnlb vmnlh vmnlf vmnlg vmal vmalb vmalhw vmalf vmah vmahb vmahh
	    vmahf vmalh vmalhb vmalhh vmalhf vmae vmaeb vmaeh vmaef vmale
	    vmaleb vmaleh vmalef vmao vmaob vmaoh vmaof vmalo vmalob vmaloh
	    vmalof vmh vmhb vmhh vmhf vmlh vmlhb vmlhh vmlhf vml vmlb vmlhw
	    vmlf vme vmeb vmeh vmef vmle vmleb vmleh vmlef vmo vmob vmoh vmof
	    vmlo vmlob vmloh vmlof vno vnot vo vpopct verllv verllvb verllvh
	    verllvf verllvg verll verllb verllh verllf verllg verim verimb
	    verimh verimf verimg veslv veslvb veslvh veslvf veslvg vesl veslb
	    veslh veslf veslg vesrav vesravb vesravh vesravf vesravg vesra
	    vesrab vesrah vesraf vesrag vesrlv vesrlvb vesrlvh vesrlvf vesrlvg
	    vesrl vesrlb vesrlh vesrlf vesrlg vsl vslb vsldb vsra vsrab vsrl
	    vsrlb vs vsb vsh vsf vsg vsq vscbi vscbib vscbih vscbif vscbig
	    vscbiq vsbi vsbiq vsbcbi vsbcbiq vsumg vsumgh vsumgf vsumq vsumqf
	    vsumqg vsum vsumb vsumh vtm vfae vfaeb vfaeh vfaef vfaebs vfaehs
	    vfaefs vfaezb vfaezh vfaezf vfaezbs vfaezhs vfaezfs vfee vfeeb
	    vfeeh vfeef vfeebs vfeehs vfeefs vfeezb vfeezh vfeezf vfeezbs
	    vfeezhs vfeezfs vfene vfeneb vfeneh vfenef vfenebs vfenehs vfenefs
	    vfenezb vfenezh vfenezf vfenezbs vfenezhs vfenezfs vistr vistrb
	    vistrh vistrf vistrbs vistrhs vistrfs vstrc vstrcb vstrch vstrcf
	    vstrcbs vstrchs vstrcfs vstrczb vstrczh vstrczf vstrczbs vstrczhs
	    vstrczfs vfa vfadb wfadb wfc wfcdb wfk wfkdb vfce vfcedb wfcedb
	    vfcedbs wfcedbs vfch vfchdb wfchdb vfchdbs wfchdbs vfche vfchedb
	    wfchedb vfchedbs wfchedbs vcdg vcdgb wcdgb vcdlg vcdlgb wcdlgb vcgd
	    vcgdb wcgdb vclgd vclgdb wclgdb vfd vfddb wfddb vfi vfidb wfidb
	    vlde vldeb wldeb vled vledb wledb vfm vfmdb wfmdb vfma vfmadb
	    wfmadb vfms vfmsdb wfmsdb vfpso vfpsodb wfpsodb vflcdb wflcdb
	    vflndb wflndb vflpdb wflpdb vfsq vfsqdb wfsqdb vfs vfsdb wfsdb
	    vftci vftcidb wftcidb)],
	# vector-enhancements facility 1
	VXE => [qw(vbperm vllezlf vmsl vmslg vnx vnn voc vpopctb vpopcth
	    vpopctf vpopctg vfasb wfasb wfaxb wfcsb wfcxb wfksb wfkxb vfcesb
	    vfcesbs wfcesb wfcesbs wfcexb wfcexbs vfchsb vfchsbs wfchsb wfchsbs
	    wfchxb wfchxbs vfchesb vfchesbs wfchesb wfchesbs wfchexb wfchexbs
	    vfdsb wfdsb wfdxb vfisb wfisb wfixb vfll vflls wflls wflld vflr
	    vflrd wflrd wflrx vfmax vfmaxsb vfmaxdb wfmaxsb wfmaxdb wfmaxxb
	    vfmin vfminsb vfmindb wfminsb wfmindb wfminxb vfmsb wfmsb wfmxb
	    vfnma vfnms vfmasb wfmasb wfmaxb vfmssb wfmssb wfmsxb vfnmasb
	    vfnmadb wfnmasb wfnmadb wfnmaxb vfnmssb vfnmsdb wfnmssb wfnmsdb
	    wfnmsxb vfpsosb wfpsosb vflcsb wflcsb vflnsb wflnsb vflpsb wflpsb
	    vfpsoxb wfpsoxb vflcxb wflcxb vflnxb wflnxb vflpxb wflpxb vfsqsb
	    wfsqsb wfsqxb vfssb wfssb wfsxb vftcisb wftcisb wftcixb)],
	# vector-packed-decimal facility
	VXD => [qw(vlrlr vlrl vstrlr vstrl vap vcp vcvb vcvbg vcvd vcvdg vdp
	    vlip vmp vmsp vpkz vpsop vrp vsdp vsrp vsp vtp vupkz)],
);
Exporter::export_ok_tags(qw(SCF GE EI MI1 MSA MSA4 MSA5 MSA8 MSA9 VX VXE VXD));

our $AUTOLOAD;

my $GR='(?:%r)?([0-9]|1[0-5])';
my $VR='(?:%v)?([0-9]|1[0-9]|2[0-9]|3[0-1])';

my ($file,$out);

sub PERLASM_BEGIN
{
	($file,$out)=(shift,"");
}
sub PERLASM_END
{
	if (defined($file)) {
		open(my $fd,'>',$file)||die("can't open $file: $!");
		print({$fd}$out);
		close($fd);
	} else {
		print($out);
	}
}

sub AUTOLOAD {
	confess(err("PARSE")) if (grep(!defined($_),@_));
	my $token;
	for ($AUTOLOAD) {
		$token=lc(".$1") if (/^.*::([A-Z_]+)$/);# uppercase: directive
		$token="\t$1" if (/^.*::([a-z]+)$/);	# lowercase: mnemonic
		confess(err("PARSE")) if (!defined($token));
	}
	$token.="\t" if ($#_>=0);
	$out.=$token.join(',',@_)."\n";
}

sub LABEL {						# label directive
	confess(err("ARGNUM")) if ($#_!=0);
	my ($label)=@_;
	$out.="$label:\n";
}

sub INCLUDE {
	confess(err("ARGNUM")) if ($#_!=0);
	my ($file)=@_;
	$out.="#include \"$file\"\n";
}

#
# Mnemonics
#

sub stfle {
	confess(err("ARGNUM")) if ($#_!=0);
	S(0xb2b0,@_);
}

sub stck {
	confess(err("ARGNUM")) if ($#_!=0);
	S(0xb205,@_);
}

# store-clock-fast facility

sub stckf {
	confess(err("ARGNUM")) if ($#_!=0);
	S(0xb27c,@_);
}

# extended-immediate facility

sub clfi {
	confess(err("ARGNUM")) if ($#_!=1);
	RILa(0xc2f,@_);
}

sub clgfi {
	confess(err("ARGNUM")) if ($#_!=1);
	RILa(0xc2e,@_);
}

sub lt {
	confess(err("ARGNUM")) if ($#_!=1);
	RXYa(0xe312,@_);
}

# general-instruction-extension facility

sub risbg {
	confess(err("ARGNUM")) if ($#_<3||$#_>4);
	RIEf(0xec55,@_);
}

# miscellaneous-instruction-extensions facility 1

sub risbgn {
	confess(err("ARGNUM")) if ($#_<3||$#_>4);
	RIEf(0xec59,@_);
}

# MSA

sub kmac {
	confess(err("ARGNUM")) if ($#_!=1);
	RRE(0xb91e,@_);
}

sub km {
	confess(err("ARGNUM")) if ($#_!=1);
	RRE(0xb92e,@_);
}

sub kmc {
	confess(err("ARGNUM")) if ($#_!=1);
	RRE(0xb92f,@_);
}

sub kimd {
	confess(err("ARGNUM")) if ($#_!=1);
	RRE(0xb93e,@_);
}

sub klmd {
	confess(err("ARGNUM")) if ($#_!=1);
	RRE(0xb93f,@_);
}

# MSA4

sub kmf {
	confess(err("ARGNUM")) if ($#_!=1);
	RRE(0xb92a,@_);
}

sub kmo {
	confess(err("ARGNUM")) if ($#_!=1);
	RRE(0xb92b,@_);
}

sub pcc {
	confess(err("ARGNUM")) if ($#_!=-1);
	RRE(0xb92c,@_);
}

sub kmctr {
	confess(err("ARGNUM")) if ($#_!=2);
	RRFb(0xb92d,@_);
}

# MSA5

sub prno {
	ppno(@_);
}

sub ppno {						# deprecated, use prno
	confess(err("ARGNUM")) if ($#_!=1);
	RRE(0xb93c,@_);
}

# MSA8

sub kma {
	confess(err("ARGNUM")) if ($#_!=2);
	RRFb(0xb929,@_);
}

# MSA9

sub kdsa {
	confess(err("ARGNUM")) if ($#_!=1);
	RRE(0xb93a,@_);
}

# VX - Support Instructions

sub vgef {
	confess(err("ARGNUM")) if ($#_!=2);
	VRV(0xe713,@_);
}
sub vgeg {
	confess(err("ARGNUM")) if ($#_!=2);
	VRV(0xe712,@_);
}

sub vgbm {
	confess(err("ARGNUM")) if ($#_!=1);
	VRIa(0xe744,@_);
}
sub vzero {
	vgbm(@_,0);
}
sub vone {
	vgbm(@_,0xffff);
}

sub vgm {
	confess(err("ARGNUM")) if ($#_!=3);
	VRIb(0xe746,@_);
}
sub vgmb {
	vgm(@_,0);
}
sub vgmh {
	vgm(@_,1);
}
sub vgmf {
	vgm(@_,2);
}
sub vgmg {
	vgm(@_,3);
}

sub vl {
	confess(err("ARGNUM")) if ($#_<1||$#_>2);
	VRX(0xe706,@_);
}

sub vlr {
	confess(err("ARGNUM")) if ($#_!=1);
	VRRa(0xe756,@_);
}

sub vlrep {
	confess(err("ARGNUM")) if ($#_!=2);
	VRX(0xe705,@_);
}
sub vlrepb {
	vlrep(@_,0);
}
sub vlreph {
	vlrep(@_,1);
}
sub vlrepf {
	vlrep(@_,2);
}
sub vlrepg {
	vlrep(@_,3);
}

sub vleb {
	confess(err("ARGNUM")) if ($#_!=2);
	VRX(0xe700,@_);
}
sub vleh {
	confess(err("ARGNUM")) if ($#_!=2);
	VRX(0xe701,@_);
}
sub vlef {
	confess(err("ARGNUM")) if ($#_!=2);
	VRX(0xe703,@_);
}
sub vleg {
	confess(err("ARGNUM")) if ($#_!=2);
	VRX(0xe702,@_);
}

sub vleib {
	confess(err("ARGNUM")) if ($#_!=2);
	VRIa(0xe740,@_);
}
sub vleih {
	confess(err("ARGNUM")) if ($#_!=2);
	VRIa(0xe741,@_);
}
sub vleif {
	confess(err("ARGNUM")) if ($#_!=2);
	VRIa(0xe743,@_);
}
sub vleig {
	confess(err("ARGNUM")) if ($#_!=2);
	VRIa(0xe742,@_);
}

sub vlgv {
	confess(err("ARGNUM")) if ($#_!=3);
	VRSc(0xe721,@_);
}
sub vlgvb {
	vlgv(@_,0);
}
sub vlgvh {
	vlgv(@_,1);
}
sub vlgvf {
	vlgv(@_,2);
}
sub vlgvg {
	vlgv(@_,3);
}

sub vllez {
	confess(err("ARGNUM")) if ($#_!=2);
	VRX(0xe704,@_);
}
sub vllezb {
	vllez(@_,0);
}
sub vllezh {
	vllez(@_,1);
}
sub vllezf {
	vllez(@_,2);
}
sub vllezg {
	vllez(@_,3);
}

sub vlm {
	confess(err("ARGNUM")) if ($#_<2||$#_>3);
	VRSa(0xe736,@_);
}

sub vlbb {
	confess(err("ARGNUM")) if ($#_!=2);
	VRX(0xe707,@_);
}

sub vlvg {
	confess(err("ARGNUM")) if ($#_!=3);
	VRSb(0xe722,@_);
}
sub vlvgb {
	vlvg(@_,0);
}
sub vlvgh {
	vlvg(@_,1);
}
sub vlvgf {
	vlvg(@_,2);
}
sub vlvgg {
	vlvg(@_,3);
}

sub vlvgp {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRf(0xe762,@_);
}

sub vll {
	confess(err("ARGNUM")) if ($#_!=2);
	VRSb(0xe737,@_);
}

sub vmrh {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe761,@_);
}
sub vmrhb {
	vmrh(@_,0);
}
sub vmrhh {
	vmrh(@_,1);
}
sub vmrhf {
	vmrh(@_,2);
}
sub vmrhg {
	vmrh(@_,3);
}

sub vmrl {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe760,@_);
}
sub vmrlb {
	vmrl(@_,0);
}
sub vmrlh {
	vmrl(@_,1);
}
sub vmrlf {
	vmrl(@_,2);
}
sub vmrlg {
	vmrl(@_,3);
}

sub vpk {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe794,@_);
}
sub vpkh {
	vpk(@_,1);
}
sub vpkf {
	vpk(@_,2);
}
sub vpkg {
	vpk(@_,3);
}

sub vpks {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRb(0xe797,@_);
}
sub vpksh {
	vpks(@_,1,0);
}
sub vpksf {
	vpks(@_,2,0);
}
sub vpksg {
	vpks(@_,3,0);
}
sub vpkshs {
	vpks(@_,1,1);
}
sub vpksfs {
	vpks(@_,2,1);
}
sub vpksgs {
	vpks(@_,3,1);
}

sub vpkls {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRb(0xe795,@_);
}
sub vpklsh {
	vpkls(@_,1,0);
}
sub vpklsf {
	vpkls(@_,2,0);
}
sub vpklsg {
	vpkls(@_,3,0);
}
sub vpklshs {
	vpkls(@_,1,1);
}
sub vpklsfs {
	vpkls(@_,2,1);
}
sub vpklsgs {
	vpkls(@_,3,1);
}

sub vperm {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRe(0xe78c,@_);
}

sub vpdi {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe784,@_);
}

sub vrep {
	confess(err("ARGNUM")) if ($#_!=3);
	VRIc(0xe74d,@_);
}
sub vrepb {
	vrep(@_,0);
}
sub vreph {
	vrep(@_,1);
}
sub vrepf {
	vrep(@_,2);
}
sub vrepg {
	vrep(@_,3);
}

sub vrepi {
	confess(err("ARGNUM")) if ($#_!=2);
	VRIa(0xe745,@_);
}
sub vrepib {
	vrepi(@_,0);
}
sub vrepih {
	vrepi(@_,1);
}
sub vrepif {
	vrepi(@_,2);
}
sub vrepig {
	vrepi(@_,3);
}

sub vscef {
	confess(err("ARGNUM")) if ($#_!=2);
	VRV(0xe71b,@_);
}
sub vsceg {
	confess(err("ARGNUM")) if ($#_!=2);
	VRV(0xe71a,@_);
}

sub vsel {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRe(0xe78d,@_);
}

sub vseg {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRa(0xe75f,@_);
}
sub vsegb {
	vseg(@_,0);
}
sub vsegh {
	vseg(@_,1);
}
sub vsegf {
	vseg(@_,2);
}

sub vst {
	confess(err("ARGNUM")) if ($#_<1||$#_>2);
	VRX(0xe70e,@_);
}

sub vsteb {
	confess(err("ARGNUM")) if ($#_!=2);
	VRX(0xe708,@_);
}
sub vsteh {
	confess(err("ARGNUM")) if ($#_!=2);
	VRX(0xe709,@_);
}
sub vstef {
	confess(err("ARGNUM")) if ($#_!=2);
	VRX(0xe70b,@_);
}
sub vsteg {
	confess(err("ARGNUM")) if ($#_!=2);
	VRX(0xe70a,@_);
}

sub vstm {
	confess(err("ARGNUM")) if ($#_<2||$#_>3);
	VRSa(0xe73e,@_);
}

sub vstl {
	confess(err("ARGNUM")) if ($#_!=2);
	VRSb(0xe73f,@_);
}

sub vuph {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRa(0xe7d7,@_);
}
sub vuphb {
	vuph(@_,0);
}
sub vuphh {
	vuph(@_,1);
}
sub vuphf {
	vuph(@_,2);
}

sub vuplh {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRa(0xe7d5,@_);
}
sub vuplhb {
	vuplh(@_,0);
}
sub vuplhh {
	vuplh(@_,1);
}
sub vuplhf {
	vuplh(@_,2);
}

sub vupl {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRa(0xe7d6,@_);
}
sub vuplb {
	vupl(@_,0);
}
sub vuplhw {
	vupl(@_,1);
}
sub vuplf {
	vupl(@_,2);
}

sub vupll {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRa(0xe7d4,@_);
}
sub vupllb {
	vupll(@_,0);
}
sub vupllh {
	vupll(@_,1);
}
sub vupllf {
	vupll(@_,2);
}

# VX - Integer Instructions

sub va {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe7f3,@_);
}
sub vab {
	va(@_,0);
}
sub vah {
	va(@_,1);
}
sub vaf {
	va(@_,2);
}
sub vag {
	va(@_,3);
}
sub vaq {
	va(@_,4);
}

sub vacc {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe7f1,@_);
}
sub vaccb {
	vacc(@_,0);
}
sub vacch {
	vacc(@_,1);
}
sub vaccf {
	vacc(@_,2);
}
sub vaccg {
	vacc(@_,3);
}
sub vaccq {
	vacc(@_,4);
}

sub vac {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRd(0xe7bb,@_);
}
sub vacq {
	vac(@_,4);
}

sub vaccc {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRd(0xe7b9,@_);
}
sub vacccq {
	vaccc(@_,4);
}

sub vn {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRc(0xe768,@_);
}

sub vnc {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRc(0xe769,@_);
}

sub vavg {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe7f2,@_);
}
sub vavgb {
	vavg(@_,0);
}
sub vavgh {
	vavg(@_,1);
}
sub vavgf {
	vavg(@_,2);
}
sub vavgg {
	vavg(@_,3);
}

sub vavgl {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe7f0,@_);
}
sub vavglb {
	vavgl(@_,0);
}
sub vavglh {
	vavgl(@_,1);
}
sub vavglf {
	vavgl(@_,2);
}
sub vavglg {
	vavgl(@_,3);
}

sub vcksm {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRc(0xe766,@_);
}

sub vec_ {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRa(0xe7db,@_);
}
sub vecb {
	vec_(@_,0);
}
sub vech {
	vec_(@_,1);
}
sub vecf {
	vec_(@_,2);
}
sub vecg {
	vec_(@_,3);
}

sub vecl {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRa(0xe7d9,@_);
}
sub veclb {
	vecl(@_,0);
}
sub veclh {
	vecl(@_,1);
}
sub veclf {
	vecl(@_,2);
}
sub veclg {
	vecl(@_,3);
}

sub vceq {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRb(0xe7f8,@_);
}
sub vceqb {
	vceq(@_,0,0);
}
sub vceqh {
	vceq(@_,1,0);
}
sub vceqf {
	vceq(@_,2,0);
}
sub vceqg {
	vceq(@_,3,0);
}
sub vceqbs {
	vceq(@_,0,1);
}
sub vceqhs {
	vceq(@_,1,1);
}
sub vceqfs {
	vceq(@_,2,1);
}
sub vceqgs {
	vceq(@_,3,1);
}

sub vch {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRb(0xe7fb,@_);
}
sub vchb {
	vch(@_,0,0);
}
sub vchh {
	vch(@_,1,0);
}
sub vchf {
	vch(@_,2,0);
}
sub vchg {
	vch(@_,3,0);
}
sub vchbs {
	vch(@_,0,1);
}
sub vchhs {
	vch(@_,1,1);
}
sub vchfs {
	vch(@_,2,1);
}
sub vchgs {
	vch(@_,3,1);
}

sub vchl {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRb(0xe7f9,@_);
}
sub vchlb {
	vchl(@_,0,0);
}
sub vchlh {
	vchl(@_,1,0);
}
sub vchlf {
	vchl(@_,2,0);
}
sub vchlg {
	vchl(@_,3,0);
}
sub vchlbs {
	vchl(@_,0,1);
}
sub vchlhs {
	vchl(@_,1,1);
}
sub vchlfs {
	vchl(@_,2,1);
}
sub vchlgs {
	vchl(@_,3,1);
}

sub vclz {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRa(0xe753,@_);
}
sub vclzb {
	vclz(@_,0);
}
sub vclzh {
	vclz(@_,1);
}
sub vclzf {
	vclz(@_,2);
}
sub vclzg {
	vclz(@_,3);
}

sub vctz {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRa(0xe752,@_);
}
sub vctzb {
	vctz(@_,0);
}
sub vctzh {
	vctz(@_,1);
}
sub vctzf {
	vctz(@_,2);
}
sub vctzg {
	vctz(@_,3);
}

sub vx {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRc(0xe76d,@_);
}

sub vgfm {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe7b4,@_);
}
sub vgfmb {
	vgfm(@_,0);
}
sub vgfmh {
	vgfm(@_,1);
}
sub vgfmf {
	vgfm(@_,2);
}
sub vgfmg {
	vgfm(@_,3);
}

sub vgfma {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRd(0xe7bc,@_);
}
sub vgfmab {
	vgfma(@_,0);
}
sub vgfmah {
	vgfma(@_,1);
}
sub vgfmaf {
	vgfma(@_,2);
}
sub vgfmag {
	vgfma(@_,3);
}

sub vlc {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRa(0xe7de,@_);
}
sub vlcb {
	vlc(@_,0);
}
sub vlch {
	vlc(@_,1);
}
sub vlcf {
	vlc(@_,2);
}
sub vlcg {
	vlc(@_,3);
}

sub vlp {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRa(0xe7df,@_);
}
sub vlpb {
	vlp(@_,0);
}
sub vlph {
	vlp(@_,1);
}
sub vlpf {
	vlp(@_,2);
}
sub vlpg {
	vlp(@_,3);
}

sub vmx {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe7ff,@_);
}
sub vmxb {
	vmx(@_,0);
}
sub vmxh {
	vmx(@_,1);
}
sub vmxf {
	vmx(@_,2);
}
sub vmxg {
	vmx(@_,3);
}

sub vmxl {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe7fd,@_);
}
sub vmxlb {
	vmxl(@_,0);
}
sub vmxlh {
	vmxl(@_,1);
}
sub vmxlf {
	vmxl(@_,2);
}
sub vmxlg {
	vmxl(@_,3);
}

sub vmn {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe7fe,@_);
}
sub vmnb {
	vmn(@_,0);
}
sub vmnh {
	vmn(@_,1);
}
sub vmnf {
	vmn(@_,2);
}
sub vmng {
	vmn(@_,3);
}

sub vmnl {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe7fc,@_);
}
sub vmnlb {
	vmnl(@_,0);
}
sub vmnlh {
	vmnl(@_,1);
}
sub vmnlf {
	vmnl(@_,2);
}
sub vmnlg {
	vmnl(@_,3);
}

sub vmal {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRd(0xe7aa,@_);
}
sub vmalb {
	vmal(@_,0);
}
sub vmalhw {
	vmal(@_,1);
}
sub vmalf {
	vmal(@_,2);
}

sub vmah {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRd(0xe7ab,@_);
}
sub vmahb {
	vmah(@_,0);
}
sub vmahh {
	vmah(@_,1);
}
sub vmahf {
	vmah(@_,2);
}

sub vmalh {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRd(0xe7a9,@_);
}
sub vmalhb {
	vmalh(@_,0);
}
sub vmalhh {
	vmalh(@_,1);
}
sub vmalhf {
	vmalh(@_,2);
}

sub vmae {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRd(0xe7ae,@_);
}
sub vmaeb {
	vmae(@_,0);
}
sub vmaeh {
	vmae(@_,1);
}
sub vmaef {
	vmae(@_,2);
}

sub vmale {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRd(0xe7ac,@_);
}
sub vmaleb {
	vmale(@_,0);
}
sub vmaleh {
	vmale(@_,1);
}
sub vmalef {
	vmale(@_,2);
}

sub vmao {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRd(0xe7af,@_);
}
sub vmaob {
	vmao(@_,0);
}
sub vmaoh {
	vmao(@_,1);
}
sub vmaof {
	vmao(@_,2);
}

sub vmalo {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRd(0xe7ad,@_);
}
sub vmalob {
	vmalo(@_,0);
}
sub vmaloh {
	vmalo(@_,1);
}
sub vmalof {
	vmalo(@_,2);
}

sub vmh {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe7a3,@_);
}
sub vmhb {
	vmh(@_,0);
}
sub vmhh {
	vmh(@_,1);
}
sub vmhf {
	vmh(@_,2);
}

sub vmlh {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe7a1,@_);
}
sub vmlhb {
	vmlh(@_,0);
}
sub vmlhh {
	vmlh(@_,1);
}
sub vmlhf {
	vmlh(@_,2);
}

sub vml {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe7a2,@_);
}
sub vmlb {
	vml(@_,0);
}
sub vmlhw {
	vml(@_,1);
}
sub vmlf {
	vml(@_,2);
}

sub vme {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe7a6,@_);
}
sub vmeb {
	vme(@_,0);
}
sub vmeh {
	vme(@_,1);
}
sub vmef {
	vme(@_,2);
}

sub vmle {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe7a4,@_);
}
sub vmleb {
	vmle(@_,0);
}
sub vmleh {
	vmle(@_,1);
}
sub vmlef {
	vmle(@_,2);
}

sub vmo {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe7a7,@_);
}
sub vmob {
	vmo(@_,0);
}
sub vmoh {
	vmo(@_,1);
}
sub vmof {
	vmo(@_,2);
}

sub vmlo {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe7a5,@_);
}
sub vmlob {
	vmlo(@_,0);
}
sub vmloh {
	vmlo(@_,1);
}
sub vmlof {
	vmlo(@_,2);
}

sub vno {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRc(0xe76b,@_);
}
sub vnot {
	vno(@_,$_[1]);
}

sub vo {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRc(0xe76a,@_);
}

sub vpopct {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRa(0xe750,@_);
}

sub verllv {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe773,@_);
}
sub verllvb {
	verllv(@_,0);
}
sub verllvh {
	verllv(@_,1);
}
sub verllvf {
	verllv(@_,2);
}
sub verllvg {
	verllv(@_,3);
}

sub verll {
	confess(err("ARGNUM")) if ($#_!=3);
	VRSa(0xe733,@_);
}
sub verllb {
	verll(@_,0);
}
sub verllh {
	verll(@_,1);
}
sub verllf {
	verll(@_,2);
}
sub verllg {
	verll(@_,3);
}

sub verim {
	confess(err("ARGNUM")) if ($#_!=4);
	VRId(0xe772,@_);
}
sub verimb {
	verim(@_,0);
}
sub verimh {
	verim(@_,1);
}
sub verimf {
	verim(@_,2);
}
sub verimg {
	verim(@_,3);
}

sub veslv {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe770,@_);
}
sub veslvb {
	veslv(@_,0);
}
sub veslvh {
	veslv(@_,1);
}
sub veslvf {
	veslv(@_,2);
}
sub veslvg {
	veslv(@_,3);
}

sub vesl {
	confess(err("ARGNUM")) if ($#_!=3);
	VRSa(0xe730,@_);
}
sub veslb {
	vesl(@_,0);
}
sub veslh {
	vesl(@_,1);
}
sub veslf {
	vesl(@_,2);
}
sub veslg {
	vesl(@_,3);
}

sub vesrav {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe77a,@_);
}
sub vesravb {
	vesrav(@_,0);
}
sub vesravh {
	vesrav(@_,1);
}
sub vesravf {
	vesrav(@_,2);
}
sub vesravg {
	vesrav(@_,3);
}

sub vesra {
	confess(err("ARGNUM")) if ($#_!=3);
	VRSa(0xe73a,@_);
}
sub vesrab {
	vesra(@_,0);
}
sub vesrah {
	vesra(@_,1);
}
sub vesraf {
	vesra(@_,2);
}
sub vesrag {
	vesra(@_,3);
}

sub vesrlv {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe778,@_);
}
sub vesrlvb {
	vesrlv(@_,0);
}
sub vesrlvh {
	vesrlv(@_,1);
}
sub vesrlvf {
	vesrlv(@_,2);
}
sub vesrlvg {
	vesrlv(@_,3);
}

sub vesrl {
	confess(err("ARGNUM")) if ($#_!=3);
	VRSa(0xe738,@_);
}
sub vesrlb {
	vesrl(@_,0);
}
sub vesrlh {
	vesrl(@_,1);
}
sub vesrlf {
	vesrl(@_,2);
}
sub vesrlg {
	vesrl(@_,3);
}

sub vsl {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRc(0xe774,@_);
}

sub vslb {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRc(0xe775,@_);
}

sub vsldb {
	confess(err("ARGNUM")) if ($#_!=3);
	VRId(0xe777,@_);
}

sub vsra {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRc(0xe77e,@_);
}

sub vsrab {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRc(0xe77f,@_);
}

sub vsrl {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRc(0xe77c,@_);
}

sub vsrlb {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRc(0xe77d,@_);
}

sub vs {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe7f7,@_);
}
sub vsb {
	vs(@_,0);
}
sub vsh {
	vs(@_,1);
}
sub vsf {
	vs(@_,2);
}
sub vsg {
	vs(@_,3);
}
sub vsq {
	vs(@_,4);
}

sub vscbi {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe7f5,@_);
}
sub vscbib {
	vscbi(@_,0);
}
sub vscbih {
	vscbi(@_,1);
}
sub vscbif {
	vscbi(@_,2);
}
sub vscbig {
	vscbi(@_,3);
}
sub vscbiq {
	vscbi(@_,4);
}

sub vsbi {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRd(0xe7bf,@_);
}
sub vsbiq {
	vsbi(@_,4);
}

sub vsbcbi {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRd(0xe7bd,@_);
}
sub vsbcbiq {
	vsbcbi(@_,4);
}

sub vsumg {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe765,@_);
}
sub vsumgh {
	vsumg(@_,1);
}
sub vsumgf {
	vsumg(@_,2);
}

sub vsumq {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe767,@_);
}
sub vsumqf {
	vsumq(@_,2);
}
sub vsumqg {
	vsumq(@_,3);
}

sub vsum {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRc(0xe764,@_);
}
sub vsumb {
	vsum(@_,0);
}
sub vsumh {
	vsum(@_,1);
}

sub vtm {
	confess(err("ARGNUM")) if ($#_!=1);
	VRRa(0xe7d8,@_);
}

# VX - String Instructions

sub vfae {
	confess(err("ARGNUM")) if ($#_<3||$#_>4);
	VRRb(0xe782,@_);
}
sub vfaeb {
	vfae(@_[0..2],0,$_[3]);
}
sub vfaeh {
	vfae(@_[0..2],1,$_[3]);
}
sub vfaef {
	vfae(@_[0..2],2,$_[3]);
}
sub vfaebs {
	$_[3]=0 if (!defined($_[3]));
	vfae(@_[0..2],0,0x1|$_[3]);
}
sub vfaehs {
	$_[3]=0 if (!defined($_[3]));
	vfae(@_[0..2],1,0x1|$_[3]);
}
sub vfaefs {
	$_[3]=0 if (!defined($_[3]));
	vfae(@_[0..2],2,0x1|$_[3]);
}
sub vfaezb {
	$_[3]=0 if (!defined($_[3]));
	vfae(@_[0..2],0,0x2|$_[3]);
}
sub vfaezh {
	$_[3]=0 if (!defined($_[3]));
	vfae(@_[0..2],1,0x2|$_[3]);
}
sub vfaezf {
	$_[3]=0 if (!defined($_[3]));
	vfae(@_[0..2],2,0x2|$_[3]);
}
sub vfaezbs {
	$_[3]=0 if (!defined($_[3]));
	vfae(@_[0..2],0,0x3|$_[3]);
}
sub vfaezhs {
	$_[3]=0 if (!defined($_[3]));
	vfae(@_[0..2],1,0x3|$_[3]);
}
sub vfaezfs {
	$_[3]=0 if (!defined($_[3]));
	vfae(@_[0..2],2,0x3|$_[3]);
}

sub vfee {
	confess(err("ARGNUM")) if ($#_<3||$#_>4);
	VRRb(0xe780,@_);
}
sub vfeeb {
	vfee(@_[0..2],0,$_[3]);
}
sub vfeeh {
	vfee(@_[0..2],1,$_[3]);
}
sub vfeef {
	vfee(@_[0..2],2,$_[3]);
}
sub vfeebs {
	vfee(@_,0,1);
}
sub vfeehs {
	vfee(@_,1,1);
}
sub vfeefs {
	vfee(@_,2,1);
}
sub vfeezb {
	vfee(@_,0,2);
}
sub vfeezh {
	vfee(@_,1,2);
}
sub vfeezf {
	vfee(@_,2,2);
}
sub vfeezbs {
	vfee(@_,0,3);
}
sub vfeezhs {
	vfee(@_,1,3);
}
sub vfeezfs {
	vfee(@_,2,3);
}

sub vfene {
	confess(err("ARGNUM")) if ($#_<3||$#_>4);
	VRRb(0xe781,@_);
}
sub vfeneb {
	vfene(@_[0..2],0,$_[3]);
}
sub vfeneh {
	vfene(@_[0..2],1,$_[3]);
}
sub vfenef {
	vfene(@_[0..2],2,$_[3]);
}
sub vfenebs {
	vfene(@_,0,1);
}
sub vfenehs {
	vfene(@_,1,1);
}
sub vfenefs {
	vfene(@_,2,1);
}
sub vfenezb {
	vfene(@_,0,2);
}
sub vfenezh {
	vfene(@_,1,2);
}
sub vfenezf {
	vfene(@_,2,2);
}
sub vfenezbs {
	vfene(@_,0,3);
}
sub vfenezhs {
	vfene(@_,1,3);
}
sub vfenezfs {
	vfene(@_,2,3);
}

sub vistr {
	confess(err("ARGNUM")) if ($#_<2||$#_>3);
	VRRa(0xe75c,@_[0..2],0,$_[3]);
}
sub vistrb {
	vistr(@_[0..1],0,$_[2]);
}
sub vistrh {
	vistr(@_[0..1],1,$_[2]);
}
sub vistrf {
	vistr(@_[0..1],2,$_[2]);
}
sub vistrbs {
	vistr(@_,0,1);
}
sub vistrhs {
	vistr(@_,1,1);
}
sub vistrfs {
	vistr(@_,2,1);
}

sub vstrc {
	confess(err("ARGNUM")) if ($#_<4||$#_>5);
	VRRd(0xe78a,@_);
}
sub vstrcb {
	vstrc(@_[0..3],0,$_[4]);
}
sub vstrch {
	vstrc(@_[0..3],1,$_[4]);
}
sub vstrcf {
	vstrc(@_[0..3],2,$_[4]);
}
sub vstrcbs {
	$_[4]=0 if (!defined($_[4]));
	vstrc(@_[0..3],0,0x1|$_[4]);
}
sub vstrchs {
	$_[4]=0 if (!defined($_[4]));
	vstrc(@_[0..3],1,0x1|$_[4]);
}
sub vstrcfs {
	$_[4]=0 if (!defined($_[4]));
	vstrc(@_[0..3],2,0x1|$_[4]);
}
sub vstrczb {
	$_[4]=0 if (!defined($_[4]));
	vstrc(@_[0..3],0,0x2|$_[4]);
}
sub vstrczh {
	$_[4]=0 if (!defined($_[4]));
	vstrc(@_[0..3],1,0x2|$_[4]);
}
sub vstrczf {
	$_[4]=0 if (!defined($_[4]));
	vstrc(@_[0..3],2,0x2|$_[4]);
}
sub vstrczbs {
	$_[4]=0 if (!defined($_[4]));
	vstrc(@_[0..3],0,0x3|$_[4]);
}
sub vstrczhs {
	$_[4]=0 if (!defined($_[4]));
	vstrc(@_[0..3],1,0x3|$_[4]);
}
sub vstrczfs {
	$_[4]=0 if (!defined($_[4]));
	vstrc(@_[0..3],2,0x3|$_[4]);
}

# VX - Floating-point Instructions

sub vfa {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRc(0xe7e3,@_);
}
sub vfadb {
	vfa(@_,3,0);
}
sub wfadb {
	vfa(@_,3,8);
}

sub wfc {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRa(0xe7cb,@_);
}
sub wfcdb {
	wfc(@_,3,0);
}

sub wfk {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRa(0xe7ca,@_);
}
sub wfksb {
	wfk(@_,2,0);
}
sub wfkdb {
	wfk(@_,3,0);
}
sub wfkxb {
	wfk(@_,4,0);
}

sub vfce {
	confess(err("ARGNUM")) if ($#_!=5);
	VRRc(0xe7e8,@_);
}
sub vfcedb {
	vfce(@_,3,0,0);
}
sub vfcedbs {
	vfce(@_,3,0,1);
}
sub wfcedb {
	vfce(@_,3,8,0);
}
sub wfcedbs {
	vfce(@_,3,8,1);
}

sub vfch {
	confess(err("ARGNUM")) if ($#_!=5);
	VRRc(0xe7eb,@_);
}
sub vfchdb {
	vfch(@_,3,0,0);
}
sub vfchdbs {
	vfch(@_,3,0,1);
}
sub wfchdb {
	vfch(@_,3,8,0);
}
sub wfchdbs {
	vfch(@_,3,8,1);
}

sub vfche {
	confess(err("ARGNUM")) if ($#_!=5);
	VRRc(0xe7ea,@_);
}
sub vfchedb {
	vfche(@_,3,0,0);
}
sub vfchedbs {
	vfche(@_,3,0,1);
}
sub wfchedb {
	vfche(@_,3,8,0);
}
sub wfchedbs {
	vfche(@_,3,8,1);
}

sub vcdg {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRa(0xe7c3,@_);
}
sub vcdgb {
	vcdg(@_[0..1],3,@_[2..3]);
}
sub wcdgb {
	vcdg(@_[0..1],3,0x8|$_[2],$_[3]);
}

sub vcdlg {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRa(0xe7c1,@_);
}
sub vcdlgb {
	vcdlg(@_[0..1],3,@_[2..3]);
}
sub wcdlgb {
	vcdlg(@_[0..1],3,0x8|$_[2],$_[3]);
}

sub vcgd {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRa(0xe7c2,@_);
}
sub vcgdb {
	vcgd(@_[0..1],3,@_[2..3]);
}
sub wcgdb {
	vcgd(@_[0..1],3,0x8|$_[2],$_[3]);
}

sub vclgd {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRa(0xe7c0,@_);
}
sub vclgdb {
	vclgd(@_[0..1],3,@_[2..3]);
}
sub wclgdb {
	vclgd(@_[0..1],3,0x8|$_[2],$_[3]);
}

sub vfd {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRc(0xe7e5,@_);
}
sub vfddb {
	vfd(@_,3,0);
}
sub wfddb {
	vfd(@_,3,8);
}

sub vfi {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRa(0xe7c7,@_);
}
sub vfidb {
	vfi(@_[0..1],3,@_[2..3]);
}
sub wfidb {
	vfi(@_[0..1],3,0x8|$_[2],$_[3]);
}

sub vlde {	# deprecated, use vfll
	confess(err("ARGNUM")) if ($#_!=3);
	VRRa(0xe7c4,@_);
}
sub vldeb {	# deprecated, use vflls
	vlde(@_,2,0);
}
sub wldeb {	# deprecated, use wflls
	vlde(@_,2,8);
}

sub vled {	# deprecated, use vflr
	confess(err("ARGNUM")) if ($#_!=4);
	VRRa(0xe7c5,@_);
}
sub vledb {	# deprecated, use vflrd
	vled(@_[0..1],3,@_[2..3]);
}
sub wledb {	# deprecated, use wflrd
	vled(@_[0..1],3,0x8|$_[2],$_[3]);
}

sub vfm {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRc(0xe7e7,@_);
}
sub vfmdb {
	vfm(@_,3,0);
}
sub wfmdb {
	vfm(@_,3,8);
}

sub vfma {
	confess(err("ARGNUM")) if ($#_!=5);
	VRRe(0xe78f,@_);
}
sub vfmadb {
	vfma(@_,0,3);
}
sub wfmadb {
	vfma(@_,8,3);
}

sub vfms {
	confess(err("ARGNUM")) if ($#_!=5);
	VRRe(0xe78e,@_);
}
sub vfmsdb {
	vfms(@_,0,3);
}
sub wfmsdb {
	vfms(@_,8,3);
}

sub vfpso {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRa(0xe7cc,@_);
}
sub vfpsodb {
	vfpso(@_[0..1],3,0,$_[2]);
}
sub wfpsodb {
	vfpso(@_[0..1],3,8,$_[2]);
}
sub vflcdb {
	vfpso(@_,3,0,0);
}
sub wflcdb {
	vfpso(@_,3,8,0);
}
sub vflndb {
	vfpso(@_,3,0,1);
}
sub wflndb {
	vfpso(@_,3,8,1);
}
sub vflpdb {
	vfpso(@_,3,0,2);
}
sub wflpdb {
	vfpso(@_,3,8,2);
}

sub vfsq {
	confess(err("ARGNUM")) if ($#_!=3);
	VRRa(0xe7ce,@_);
}
sub vfsqdb {
	vfsq(@_,3,0);
}
sub wfsqdb {
	vfsq(@_,3,8);
}

sub vfs {
	confess(err("ARGNUM")) if ($#_!=4);
	VRRc(0xe7e2,@_);
}
sub vfsdb {
	vfs(@_,3,0);
}
sub wfsdb {
	vfs(@_,3,8);
}

sub vftci {
	confess(err("ARGNUM")) if ($#_!=4);
	VRIe(0xe74a,@_);
}
sub vftcidb {
	vftci(@_,3,0);
}
sub wftcidb {
	vftci(@_,3,8);
}

# VXE - Support Instructions

sub vbperm {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRc(0xe785,@_);
}

sub vllezlf {
	vllez(@_,6);
}

# VXE - Integer Instructions

sub vmsl {
	confess(err("ARGNUM")) if ($#_!=5);
	VRRd(0xe7b8,@_);
}
sub vmslg {
	vmsl(@_[0..3],3,$_[4]);
}

sub vnx {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRc(0xe76c,@_);
}

sub vnn {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRc(0xe76e,@_);
}

sub voc {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRc(0xe76f,@_);
}

sub vpopctb {
	vpopct(@_,0);
}
sub vpopcth {
	vpopct(@_,1);
}
sub vpopctf {
	vpopct(@_,2);
}
sub vpopctg {
	vpopct(@_,3);
}

# VXE - Floating-Point Instructions

sub vfasb {
	vfa(@_,2,0);
}
sub wfasb {
	vfa(@_,2,8);
}
sub wfaxb {
	vfa(@_,4,8);
}

sub wfcsb {
	wfc(@_,2,0);
}
sub wfcxb {
	wfc(@_,4,0);
}

sub vfcesb {
	vfce(@_,2,0,0);
}
sub vfcesbs {
	vfce(@_,2,0,1);
}
sub wfcesb {
	vfce(@_,2,8,0);
}
sub wfcesbs {
	vfce(@_,2,8,1);
}
sub wfcexb {
	vfce(@_,4,8,0);
}
sub wfcexbs {
	vfce(@_,4,8,1);
}

sub vfchsb {
	vfch(@_,2,0,0);
}
sub vfchsbs {
	vfch(@_,2,0,1);
}
sub wfchsb {
	vfch(@_,2,8,0);
}
sub wfchsbs {
	vfch(@_,2,8,1);
}
sub wfchxb {
	vfch(@_,4,8,0);
}
sub wfchxbs {
	vfch(@_,4,8,1);
}

sub vfchesb {
	vfche(@_,2,0,0);
}
sub vfchesbs {
	vfche(@_,2,0,1);
}
sub wfchesb {
	vfche(@_,2,8,0);
}
sub wfchesbs {
	vfche(@_,2,8,1);
}
sub wfchexb {
	vfche(@_,4,8,0);
}
sub wfchexbs {
	vfche(@_,4,8,1);
}

sub vfdsb {
	vfd(@_,2,0);
}
sub wfdsb {
	vfd(@_,2,8);
}
sub wfdxb {
	vfd(@_,4,8);
}

sub vfisb {
	vfi(@_[0..1],2,@_[2..3]);
}
sub wfisb {
	vfi(@_[0..1],2,0x8|$_[2],$_[3]);
}
sub wfixb {
	vfi(@_[0..1],4,0x8|$_[2],$_[3]);
}

sub vfll {
	vlde(@_);
}
sub vflls {
	vfll(@_,2,0);
}
sub wflls {
	vfll(@_,2,8);
}
sub wflld {
	vfll(@_,3,8);
}

sub vflr {
	vled(@_);
}
sub vflrd {
	vflr(@_[0..1],3,@_[2..3]);
}
sub wflrd {
	vflr(@_[0..1],3,0x8|$_[2],$_[3]);
}
sub wflrx {
	vflr(@_[0..1],4,0x8|$_[2],$_[3]);
}

sub vfmax {
	confess(err("ARGNUM")) if ($#_!=5);
	VRRc(0xe7ef,@_);
}
sub vfmaxsb {
	vfmax(@_[0..2],2,0,$_[3]);
}
sub vfmaxdb {
	vfmax(@_[0..2],3,0,$_[3]);
}
sub wfmaxsb {
	vfmax(@_[0..2],2,8,$_[3]);
}
sub wfmaxdb {
	vfmax(@_[0..2],3,8,$_[3]);
}
sub wfmaxxb {
	vfmax(@_[0..2],4,8,$_[3]);
}

sub vfmin {
	confess(err("ARGNUM")) if ($#_!=5);
	VRRc(0xe7ee,@_);
}
sub vfminsb {
	vfmin(@_[0..2],2,0,$_[5]);
}
sub vfmindb {
	vfmin(@_[0..2],3,0,$_[5]);
}
sub wfminsb {
	vfmin(@_[0..2],2,8,$_[5]);
}
sub wfmindb {
	vfmin(@_[0..2],3,8,$_[5]);
}
sub wfminxb {
	vfmin(@_[0..2],4,8,$_[5]);
}

sub vfmsb {
	vfm(@_,2,0);
}
sub wfmsb {
	vfm(@_,2,8);
}
sub wfmxb {
	vfm(@_,4,8);
}

sub vfmasb {
	vfma(@_,0,2);
}
sub wfmasb {
	vfma(@_,8,2);
}
sub wfmaxb {
	vfma(@_,8,4);
}

sub vfmssb {
	vfms(@_,0,2);
}
sub wfmssb {
	vfms(@_,8,2);
}
sub wfmsxb {
	vfms(@_,8,4);
}

sub vfnma {
	confess(err("ARGNUM")) if ($#_!=5);
	VRRe(0xe79f,@_);
}
sub vfnmasb {
	vfnma(@_,0,2);
}
sub vfnmadb {
	vfnma(@_,0,3);
}
sub wfnmasb {
	vfnma(@_,8,2);
}
sub wfnmadb {
	vfnma(@_,8,3);
}
sub wfnmaxb {
	vfnma(@_,8,4);
}

sub vfnms {
	confess(err("ARGNUM")) if ($#_!=5);
	VRRe(0xe79e,@_);
}
sub vfnmssb {
	vfnms(@_,0,2);
}
sub vfnmsdb {
	vfnms(@_,0,3);
}
sub wfnmssb {
	vfnms(@_,8,2);
}
sub wfnmsdb {
	vfnms(@_,8,3);
}
sub wfnmsxb {
	vfnms(@_,8,4);
}

sub vfpsosb {
	vfpso(@_[0..1],2,0,$_[2]);
}
sub wfpsosb {
	vfpso(@_[0..1],2,8,$_[2]);
}
sub vflcsb {
	vfpso(@_,2,0,0);
}
sub wflcsb {
	vfpso(@_,2,8,0);
}
sub vflnsb {
	vfpso(@_,2,0,1);
}
sub wflnsb {
	vfpso(@_,2,8,1);
}
sub vflpsb {
	vfpso(@_,2,0,2);
}
sub wflpsb {
	vfpso(@_,2,8,2);
}
sub vfpsoxb {
	vfpso(@_[0..1],4,0,$_[2]);
}
sub wfpsoxb {
	vfpso(@_[0..1],4,8,$_[2]);
}
sub vflcxb {
	vfpso(@_,4,0,0);
}
sub wflcxb {
	vfpso(@_,4,8,0);
}
sub vflnxb {
	vfpso(@_,4,0,1);
}
sub wflnxb {
	vfpso(@_,4,8,1);
}
sub vflpxb {
	vfpso(@_,4,0,2);
}
sub wflpxb {
	vfpso(@_,4,8,2);
}

sub vfsqsb {
	vfsq(@_,2,0);
}
sub wfsqsb {
	vfsq(@_,2,8);
}
sub wfsqxb {
	vfsq(@_,4,8);
}

sub vfssb {
	vfs(@_,2,0);
}
sub wfssb {
	vfs(@_,2,8);
}
sub wfsxb {
	vfs(@_,4,8);
}

sub vftcisb {
	vftci(@_,2,0);
}
sub wftcisb {
	vftci(@_,2,8);
}
sub wftcixb {
	vftci(@_,4,8);
}

# VXD - Support Instructions

sub vlrlr {
	confess(err("ARGNUM")) if ($#_!=2);
	VRSd(0xe637,@_);
}

sub vlrl {
	confess(err("ARGNUM")) if ($#_!=2);
	VSI(0xe635,@_);
}

sub vstrlr {
	confess(err("ARGNUM")) if ($#_!=2);
	VRSd(0xe63f,@_);
}

sub vstrl {
	confess(err("ARGNUM")) if ($#_!=2);
	VSI(0xe63d,@_);
}

sub vap {
	confess(err("ARGNUM")) if ($#_!=4);
	VRIf(0xe671,@_);
}

sub vcp {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRh(0xe677,@_);
}

sub vcvb {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRi(0xe650,@_);
}

sub vcvbg {
	confess(err("ARGNUM")) if ($#_!=2);
	VRRi(0xe652,@_);
}

sub vcvd {
	confess(err("ARGNUM")) if ($#_!=3);
	VRIi(0xe658,@_);
}

sub vcvdg {
	confess(err("ARGNUM")) if ($#_!=3);
	VRIi(0xe65a,@_);
}

sub vdp {
	confess(err("ARGNUM")) if ($#_!=4);
	VRIf(0xe67a,@_);
}

sub vlip {
	confess(err("ARGNUM")) if ($#_!=2);
	VRIh(0xe649,@_);
}

sub vmp {
	confess(err("ARGNUM")) if ($#_!=4);
	VRIf(0xe678,@_);
}

sub vmsp {
	confess(err("ARGNUM")) if ($#_!=4);
	VRIf(0xe679,@_);
}

sub vpkz {
	confess(err("ARGNUM")) if ($#_!=2);
	VSI(0xe634,@_);
}

sub vpsop {
	confess(err("ARGNUM")) if ($#_!=4);
	VRIg(0xe65b,@_);
}

sub vrp {
	confess(err("ARGNUM")) if ($#_!=4);
	VRIf(0xe67b,@_);
}

sub vsdp {
	confess(err("ARGNUM")) if ($#_!=4);
	VRIf(0xe67e,@_);
}

sub vsrp {
	confess(err("ARGNUM")) if ($#_!=4);
	VRIg(0xe659,@_);
}

sub vsp {
	confess(err("ARGNUM")) if ($#_!=4);
	VRIf(0xe673,@_);
}

sub vtp {
	confess(err("ARGNUM")) if ($#_!=0);
	VRRg(0xe65f,@_);
}

sub vupkz {
	confess(err("ARGNUM")) if ($#_!=2);
	VSI(0xe63c,@_);
}

#
# Instruction Formats
#

sub RIEf {
	confess(err("ARGNUM")) if ($#_<4||5<$#_);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$r1,$r2,$i3,$i4,$i5)=(shift,get_R(shift),get_R(shift),
					  get_I(shift,8),get_I(shift,8),
					  get_I(shift,8));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",(($opcode>>8)<<8|$r1<<4|$r2)).",";
	$out.=sprintf("%#06x",($i3<<8)|$i4).",";
	$out.=sprintf("%#06x",($i5<<8)|($opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub RILa {
	confess(err("ARGNUM")) if ($#_!=2);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$r1,$i2)=(shift,get_R(shift),get_I(shift,32));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",(($opcode>>4)<<8|$r1<<4|($opcode&0xf))).",";
	$out.=sprintf("%#06x",($i2>>16)).",";
	$out.=sprintf("%#06x",($i2&0xffff));
	$out.="\t# $memn\t$ops\n";
}

sub RRE {
	confess(err("ARGNUM")) if ($#_<0||2<$#_);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$r1,$r2)=(shift,get_R(shift),get_R(shift));

	$out.="\t.long\t".sprintf("%#010x",($opcode<<16|$r1<<4|$r2));
	$out.="\t# $memn";
	# RRE can have 0 ops e.g., pcc.
	$out.="\t$ops" if ((defined($ops))&&($ops ne ''));
	$out.="\n";
}

sub RRFb {
	confess(err("ARGNUM")) if ($#_<3||4<$#_);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$r1,$r3,$r2,$m4)=(shift,get_R(shift),get_R(shift)
	    ,get_R(shift),get_M(shift));

	$out.="\t.long\t"
	    .sprintf("%#010x",($opcode<<16|$r3<<12|$m4<<8|$r1<<4|$r2));
	$out.="\t# $memn\t$ops\n";
}

sub RXYa {
	confess(err("ARGNUM")) if ($#_!=2);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$r1,$d2,$x2,$b2)=(shift,get_R(shift),get_DXB(shift));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",(($opcode>>8)<<8|$r1<<4|$x2)).",";
	$out.=sprintf("%#06x",($b2<<12|($d2&0xfff))).",";
	$out.=sprintf("%#06x",(($d2>>12)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub S {
	confess(err("ARGNUM")) if ($#_<0||1<$#_);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$d2,$b2)=(shift,get_DB(shift));

	$out.="\t.long\t".sprintf("%#010x",($opcode<<16|$b2<<12|$d2));
	$out.="\t# $memn\t$ops\n";
}

sub VRIa {
	confess(err("ARGNUM")) if ($#_<2||3<$#_);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$v1,$i2,$m3)=(shift,get_V(shift),get_I(shift,16),
	    get_M(shift));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|($v1&0xf)<<4)).",";
	$out.=sprintf("%#06x",$i2).",";
	$out.=sprintf("%#06x",($m3<<12|RXB($v1)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub VRIb {
	confess(err("ARGNUM")) if ($#_!=4);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$v1,$i2,$i3,$m4)=(shift,get_V(shift),get_I(shift,8),
	    ,get_I(shift,8),get_M(shift));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|($v1&0xf)<<4)).",";
	$out.=sprintf("%#06x",($i2<<8|$i3)).",";
	$out.=sprintf("%#06x",($m4<<12|RXB($v1)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub VRIc {
	confess(err("ARGNUM")) if ($#_!=4);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$v1,$v3,$i2,$m4)=(shift,get_V(shift),get_V(shift),
	    ,get_I(shift,16),get_M(shift));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|($v1&0xf)<<4)|($v3&0xf)).",";
	$out.=sprintf("%#06x",$i2).",";
	$out.=sprintf("%#06x",($m4<<12|RXB($v1,$v3)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub VRId {
	confess(err("ARGNUM")) if ($#_<4||$#_>5);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$v1,$v2,$v3,$i4,$m5)=(shift,get_V(shift),get_V(shift),
	    ,get_V(shift),get_I(shift,8),get_M(shift));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|($v1&0xf)<<4)|($v2&0xf)).",";
	$out.=sprintf("%#06x",(($v3&0xf)<<12|$i4)).",";
	$out.=sprintf("%#06x",($m5<<12|RXB($v1,$v2,$v3)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub VRIe {
	confess(err("ARGNUM")) if ($#_!=5);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$v1,$v2,$i3,$m4,$m5)=(shift,get_V(shift),get_V(shift),
	    ,get_I(shift,12),get_M(shift),get_M(shift));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|($v1&0xf)<<4)|($v2&0xf)).",";
	$out.=sprintf("%#06x",($i3<<4|$m5)).",";
	$out.=sprintf("%#06x",($m4<<12|RXB($v1,$v2)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub VRIf {
	confess(err("ARGNUM")) if ($#_!=5);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$v1,$v2,$v3,$i4,$m5)=(shift,get_V(shift),get_V(shift),
	    ,get_V(shift),get_I(shift,8),get_M(shift));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|($v1&0xf)<<4)|($v2&0xf)).",";
	$out.=sprintf("%#06x",(($v3&0xf)<<12|$m5<<4)|$i4>>4).",";
	$out.=sprintf("%#06x",(($i4&0xf)<<12|RXB($v1,$v2,$v3)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub VRIg {
	confess(err("ARGNUM")) if ($#_!=5);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$v1,$v2,$i3,$i4,$m5)=(shift,get_V(shift),get_V(shift),
	    ,get_I(shift,8),get_I(shift,8),get_M(shift));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|($v1&0xf)<<4)|($v2&0xf)).",";
	$out.=sprintf("%#06x",($i4<<8|$m5<<4|$i3>>4)).",";
	$out.=sprintf("%#06x",(($i3&0xf)<<12|RXB($v1,$v2)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub VRIh {
	confess(err("ARGNUM")) if ($#_!=3);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$v1,$i2,$i3)=(shift,get_V(shift),get_I(shift,16),
	    get_I(shift,4));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|($v1&0xf)<<4)).",";
	$out.=sprintf("%#06x",$i2).",";
	$out.=sprintf("%#06x",($i3<<12|RXB($v1)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub VRIi {
	confess(err("ARGNUM")) if ($#_!=4);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$v1,$r2,$i3,$m4)=(shift,get_V(shift),get_R(shift),
	    ,get_I(shift,8),get_M(shift));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|($v1&0xf)<<4)|$r2).",";
	$out.=sprintf("%#06x",($m4<<4|$i3>>4)).",";
	$out.=sprintf("%#06x",(($i3&0xf)<<12|RXB($v1)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub VRRa {
	confess(err("ARGNUM")) if ($#_<2||5<$#_);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$v1,$v2,$m3,$m4,$m5)=(shift,get_V(shift),get_V(shift),
	    get_M(shift),get_M(shift),get_M(shift));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|($v1&0xf)<<4|($v2&0xf))).",";
	$out.=sprintf("%#06x",($m5<<4|$m4)).",";
	$out.=sprintf("%#06x",($m3<<12|RXB($v1,$v2)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub VRRb {
	confess(err("ARGNUM")) if ($#_<3||5<$#_);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$v1,$v2,$v3,$m4,$m5)=(shift,get_V(shift),get_V(shift),
	    get_V(shift),get_M(shift),get_M(shift));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|($v1&0xf)<<4|($v2&0xf))).",";
	$out.=sprintf("%#06x",(($v3&0xf)<<12|$m5<<4)).",";
	$out.=sprintf("%#06x",($m4<<12|RXB($v1,$v2,$v3)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub VRRc {
	confess(err("ARGNUM")) if ($#_<3||6<$#_);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$v1,$v2,$v3,$m4,$m5,$m6)=(shift,get_V(shift),get_V(shift),
	    get_V(shift),get_M(shift),get_M(shift),get_M(shift));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|($v1&0xf)<<4|($v2&0xf))).",";
	$out.=sprintf("%#06x",(($v3&0xf)<<12|$m6<<4|$m5)).",";
	$out.=sprintf("%#06x",($m4<<12|RXB($v1,$v2,$v3)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub VRRd {
	confess(err("ARGNUM")) if ($#_<4||6<$#_);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$v1,$v2,$v3,$v4,$m5,$m6)=(shift,get_V(shift),get_V(shift),
	    get_V(shift),get_V(shift),get_M(shift),get_M(shift));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|($v1&0xf)<<4|($v2&0xf))).",";
	$out.=sprintf("%#06x",(($v3&0xf)<<12|$m5<<8|$m6<<4)).",";
	$out.=sprintf("%#06x",(($v4&0xf)<<12|RXB($v1,$v2,$v3,$v4)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub VRRe {
	confess(err("ARGNUM")) if ($#_<4||6<$#_);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$v1,$v2,$v3,$v4,$m5,$m6)=(shift,get_V(shift),get_V(shift),
	    get_V(shift),get_V(shift),get_M(shift),get_M(shift));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|($v1&0xf)<<4|($v2&0xf))).",";
	$out.=sprintf("%#06x",(($v3&0xf)<<12|$m6<<8|$m5)).",";
	$out.=sprintf("%#06x",(($v4&0xf)<<12|RXB($v1,$v2,$v3,$v4)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub VRRf {
	confess(err("ARGNUM")) if ($#_!=3);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$v1,$r2,$r3)=(shift,get_V(shift),get_R(shift),
	    get_R(shift));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|($v1&0xf)<<4|$r2)).",";
	$out.=sprintf("%#06x",($r3<<12)).",";
	$out.=sprintf("%#06x",(RXB($v1)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub VRRg {
	confess(err("ARGNUM")) if ($#_!=1);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$v1)=(shift,get_V(shift));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|($v1&0xf))).",";
	$out.=sprintf("%#06x",0x0000).",";
	$out.=sprintf("%#06x",(RXB(0,$v1)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub VRRh {
	confess(err("ARGNUM")) if ($#_<2||$#_>3);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$v1,$v2,$m3)=(shift,get_V(shift),get_V(shift),
	    get_M(shift));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|($v1&0xf))).",";
	$out.=sprintf("%#06x",(($v2&0xf)<<12|$m3<<4)).",";
	$out.=sprintf("%#06x",(RXB(0,$v1,$v2)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub VRRi {
	confess(err("ARGNUM")) if ($#_!=3);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$r1,$v2,$m3)=(shift,get_R(shift),get_V(shift),
	    get_M(shift));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|$r1<<4|($v2&0xf))).",";
	$out.=sprintf("%#06x",($m3<<4))."\,";
	$out.=sprintf("%#06x",(RXB(0,$v2)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub VRSa {
	confess(err("ARGNUM")) if ($#_<3||$#_>4);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$v1,$v3,$d2,$b2,$m4)=(shift,get_V(shift),get_V(shift),
	    get_DB(shift),get_M(shift));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|($v1&0xf)<<4|($v3&0xf))).",";
	$out.=sprintf("%#06x",($b2<<12|$d2)).",";
	$out.=sprintf("%#06x",($m4<<12|RXB($v1,$v3)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub VRSb {
	confess(err("ARGNUM")) if ($#_<3||$#_>4);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$v1,$r3,$d2,$b2,$m4)=(shift,get_V(shift),get_R(shift),
	    get_DB(shift),get_M(shift));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|($v1&0xf)<<4|$r3)).",";
	$out.=sprintf("%#06x",($b2<<12|$d2)).",";
	$out.=sprintf("%#06x",($m4<<12|RXB($v1)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub VRSc {
	confess(err("ARGNUM")) if ($#_!=4);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$r1,$v3,$d2,$b2,$m4)=(shift,get_R(shift),get_V(shift),
	    get_DB(shift),get_M(shift));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|$r1<<4|($v3&0xf))).",";
	$out.=sprintf("%#06x",($b2<<12|$d2)).",";
	$out.=sprintf("%#06x",($m4<<12|RXB(0,$v3)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub VRSd {
	confess(err("ARGNUM")) if ($#_!=3);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$v1,$r3,$d2,$b2)=(shift,get_V(shift),get_R(shift),
	    get_DB(shift));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|$r3)).",";
	$out.=sprintf("%#06x",($b2<<12|$d2)).",";
	$out.=sprintf("%#06x",(($v1&0xf)<<12|RXB(0,0,0,$v1)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub VRV {
	confess(err("ARGNUM")) if ($#_<2||$#_>3);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$v1,$d2,$v2,$b2,$m3)=(shift,get_V(shift),get_DVB(shift),
	    get_M(shift));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|($v1&0xf)<<4|($v2&0xf))).",";
	$out.=sprintf("%#06x",($b2<<12|$d2)).",";
	$out.=sprintf("%#06x",($m3<<12|RXB($v1,$v2)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub VRX {
	confess(err("ARGNUM")) if ($#_<2||$#_>3);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$v1,$d2,$x2,$b2,$m3)=(shift,get_V(shift),get_DXB(shift),
	    get_M(shift));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|($v1&0xf)<<4|($x2))).",";
	$out.=sprintf("%#06x",($b2<<12|$d2)).",";
	$out.=sprintf("%#06x",($m3<<12|RXB($v1)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

sub VSI {
	confess(err("ARGNUM")) if ($#_!=3);
	my $ops=join(',',@_[1..$#_]);
	my $memn=(caller(1))[3];
	$memn=~s/^.*:://;
	my ($opcode,$v1,$d2,$b2,$i3)=(shift,get_V(shift),get_DB(shift),
	    get_I(shift,8));

	$out.="\t.word\t";
	$out.=sprintf("%#06x",($opcode&0xff00|$i3)).",";
	$out.=sprintf("%#06x",($b2<<12|$d2)).",";
	$out.=sprintf("%#06x",(($v1&0xf)<<12|RXB(0,0,0,$v1)<<8|$opcode&0xff));
	$out.="\t# $memn\t$ops\n";
}

#
# Internal
#

sub get_R {
	confess(err("ARGNUM")) if ($#_!=0);
	my $r;

	for (shift) {
		if (!defined) {
			$r=0;
		} elsif (/^$GR$/) {
			$r=$1;
		} else {
			confess(err("PARSE"));
		}
	}
	confess(err("ARGRANGE")) if ($r&~0xf);

	return $r;
}

sub get_V {
	confess(err("ARGNUM")) if ($#_!=0);
	my $v;

	for (shift) {
		if (!defined) {
			$v=0;
		} elsif (/^$VR$/) {
			$v=$1;
		} else {
			confess(err("PARSE"));
		}
	}
	confess(err("ARGRANGE")) if ($v&~0x1f);

	return $v;
}

sub get_I {
	confess(err("ARGNUM")) if ($#_!=1);
	my ($i,$bits)=(shift,shift);

	$i=defined($i)?(eval($i)):(0);
	confess(err("PARSE")) if (!defined($i));
	confess(err("ARGRANGE")) if (abs($i)&~(2**$bits-1));

	return $i&(2**$bits-1);
}

sub get_M {
	confess(err("ARGNUM")) if ($#_!=0);
	my $m=shift;

	$m=defined($m)?(eval($m)):(0);
	confess(err("PARSE")) if (!defined($m));
	confess(err("ARGRANGE")) if ($m&~0xf);

	return $m;
}

sub get_DB
{
	confess(err("ARGNUM")) if ($#_!=0);
	my ($d,$b);

	for (shift) {
		if (!defined) {
			($d,$b)=(0,0);
		} elsif (/^(.+)\($GR\)$/) {
			($d,$b)=(eval($1),$2);
			confess(err("PARSE")) if (!defined($d));
		} elsif (/^(.+)$/) {
			($d,$b)=(eval($1),0);
			confess(err("PARSE")) if (!defined($d));
		} else {
			confess(err("PARSE"));
		}
	}
	confess(err("ARGRANGE")) if ($d&~0xfff||$b&~0xf);

	return ($d,$b);
}

sub get_DVB
{
	confess(err("ARGNUM")) if ($#_!=0);
	my ($d,$v,$b);

	for (shift) {
		if (!defined) {
			($d,$v,$b)=(0,0,0);
		} elsif (/^(.+)\($VR,$GR\)$/) {
			($d,$v,$b)=(eval($1),$2,$3);
			confess(err("PARSE")) if (!defined($d));
		} elsif (/^(.+)\($GR\)$/) {
			($d,$v,$b)=(eval($1),0,$2);
			confess(err("PARSE")) if (!defined($d));
		} elsif (/^(.+)$/) {
			($d,$v,$b)=(eval($1),0,0);
			confess(err("PARSE")) if (!defined($d));
		} else {
			confess(err("PARSE"));
		}
	}
	confess(err("ARGRANGE")) if ($d&~0xfff||$v&~0x1f||$b&~0xf);

	return ($d,$v,$b);
}

sub get_DXB
{
	confess(err("ARGNUM")) if ($#_!=0);
	my ($d,$x,$b);

	for (shift) {
		if (!defined) {
			($d,$x,$b)=(0,0,0);
		} elsif (/^(.+)\($GR,$GR\)$/) {
			($d,$x,$b)=(eval($1),$2,$3);
			confess(err("PARSE")) if (!defined($d));
		} elsif (/^(.+)\($GR\)$/) {
			($d,$x,$b)=(eval($1),0,$2);
			confess(err("PARSE")) if (!defined($d));
		} elsif (/^(.+)$/) {
			($d,$x,$b)=(eval($1),0,0);
			confess(err("PARSE")) if (!defined($d));
		} else {
			confess(err("PARSE"));
		}
	}
	confess(err("ARGRANGE")) if ($d&~0xfff||$x&~0xf||$b&~0xf);

	return ($d,$x,$b);
}

sub RXB
{
	confess(err("ARGNUM")) if ($#_<0||3<$#_);
	my $rxb=0;

	$rxb|=0x08 if (defined($_[0])&&($_[0]&0x10));
	$rxb|=0x04 if (defined($_[1])&&($_[1]&0x10));
	$rxb|=0x02 if (defined($_[2])&&($_[2]&0x10));
	$rxb|=0x01 if (defined($_[3])&&($_[3]&0x10));

	return $rxb;
}

sub err {
	my %ERR		=
	(
		ARGNUM	=>	'Wrong number of arguments',
		ARGRANGE=>	'Argument out of range',
		PARSE	=>	'Parse error',
	);
	confess($ERR{ARGNUM}) if ($#_!=0);

	return $ERR{$_[0]};
}

1;

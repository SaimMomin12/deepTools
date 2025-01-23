import deeptools.plotFingerprint as pf

import os.path
from os import unlink


ROOT = os.path.dirname(os.path.abspath(__file__)) + "/test_data/"
BAM_IN1 = ROOT + "bowtie2_test1.bam"
FIN_PLOT1 = ROOT + "plotFingerprint_result1.png"
FIN_PLOT2 = ROOT + "plotFingerprint_result2.png"


def test_fingerprint_plot_with_minimal_options():
    """
    Test minimal command line args for fingerprint plot
    """
    out_png = '/tmp/fingerprint_plot1.png'
    args = "-b {} {} -o {}".format(BAM_IN1, BAM_IN1, out_png).split()
    pf.main(args)

    png_file_size = os.path.getsize(FIN_PLOT1)
    expected_file_size = os.path.getsize(out_png)
    size_tolerance = 15000
    size_difference = abs(png_file_size - expected_file_size)
    assert size_difference <= size_tolerance, "File size do not match"
    unlink(out_png)


def test_fingerprint_plot_with_advance_options():
    """
    Test command line args for fingerprint plot with additional options
    """
    out_rawcounts = '/tmp/rawcounts.tsv'
    out_qualitymetric = '/tmp/qualitymetrics.tsv'
    out_png2 = '/tmp/fingerprint_plot2.png'
    args = "-b {} {} -o {} --outRawCounts {} --outQualityMetrics {} -T Test_Fingerpring_Plot --JSDsample {}".format(BAM_IN1, BAM_IN1, out_png2, out_rawcounts, out_qualitymetric, BAM_IN1).split()
    pf.main(args)

    with open(out_rawcounts, "r") as _foo:
        result = len(_foo.readlines())
    _expected = 16072
    assert result == _expected, "No of lines in rawcounts files differ"

    with open(out_qualitymetric, "r") as _foo:
        result2 = len(_foo.readlines())
    _expected = 3
    assert result2 == _expected, "No of lines in quality metrics files differ"

    png_file_size = os.path.getsize(FIN_PLOT2)
    expected_file_size = os.path.getsize(out_png2)
    size_tolerance = 15000
    size_difference = abs(png_file_size - expected_file_size)
    assert size_difference <= size_tolerance, "File sizes do not match"
    unlink(out_png2)
    unlink(out_rawcounts)
    unlink(out_rawcounts)

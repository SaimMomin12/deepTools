use std::cmp;

pub fn scale_factor(
    norm_method: &str,
    mapped: u64,
    binsize: u32,
    effective_genome_size: u64,
    readlen: f32,
    _fraglen: f32,
    verbose: &bool
) -> f64 {
    let mut scale_factor = 1.0;
    match norm_method {
        "RPKM" => {
            // RPKM = # reads per tile / total reads (millions) * tile length (kb)
            let mmr = mapped as f64 / 1e6;
            let bs_kb = binsize as f64 / 1000.0;
            scale_factor *= 1.0 / (mmr * bs_kb);
        }
        "CPM" => {
            // CPM = # reads per tile / total reads (millions)
            let mmr = mapped as f64 / 1e6;
            scale_factor *= 1.0 / mmr;
        }
        "BPM" => {
            // BPM = bins per million mapped reads
            let bs_kb: f64 = binsize as f64 / 1000.0;
            let tmp_scalefactor = (mapped as f64 / bs_kb) / 1e6;
            scale_factor *= 1.0 / (tmp_scalefactor * bs_kb);
        }
        "RPGC" => {
            // RPGC = mapped reads * fragment length / effective genome size
            let tmp_scalefactor = (mapped as f64 * readlen as f64) / effective_genome_size as f64;
            scale_factor *= 1.0 / tmp_scalefactor;
        }
        _ => {}
    };
    if *verbose {
        println!("Scale factor: {}", scale_factor);
    }
    return scale_factor;
}

pub fn scale_factor_bamcompare(
    norm_method: &str,
    mapped_bam1: u64,
    mapped_bam2: u64,
    _binsize: u32,
    _effective_genome_size: u64,
    _norm: &str
) -> (f64, f64) {
    return match norm_method {
        "readCount" => {
            let min = cmp::min(mapped_bam1, mapped_bam2);
            let scale_factor1 = min as f64 / mapped_bam1 as f64;
            let scale_factor2 = min as f64 / mapped_bam2 as f64;
            (scale_factor1, scale_factor2)
        }
        "SES" => {
            // to be implemented
            (1.0, 1.0)
        }
        _ => {
            (1.0, 1.0)
        }
    }
}

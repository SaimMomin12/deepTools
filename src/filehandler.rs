use rust_htslib::bam::{Read, Reader};
use itertools::Itertools;
use std::io::{BufReader, BufWriter, Write};
use std::io::prelude::*;
use std::fs::File;
use std::path::Path;
use bigtools::{BigWigRead, BigWigWrite, Value};
use bigtools::beddata::BedParserStreamingIterator;
use std::collections::HashMap;
use flate2::write::GzEncoder;
use flate2::Compression;
use crate::computematrix::{Scalingregions, Gtfparse, Revalue, Region, Bin};
use crate::calc::{mean_float, median_float, min_float, max_float, sum_float, std_float};

pub fn bam_ispaired(bam_ifile: &str) -> bool {
    let mut bam = Reader::from_path(bam_ifile).unwrap();
    for record in bam.records() {
        let record = record.expect("Error parsing record.");
        if record.is_paired() {
            return true;
        }
    }
    return false;
}

pub fn write_covfile<LI>(lines: LI, ofile: &str, filetype: &str, chromsizes: HashMap<String, u32>)
where
    LI: Iterator<Item = (String, Value)>,
 {
    if filetype == "bedgraph" {
        // write output file, bedgraph
        let mut writer = BufWriter::new(File::create(ofile).unwrap());
        for (chrom, val) in lines {
            writeln!(writer, "{}\t{}\t{}\t{}", chrom, val.start, val.end, val.value).unwrap();
        }
    } else {
        let vals = BedParserStreamingIterator::wrap_infallible_iter(
            lines,
            false
        );
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .build()
            .expect("Unable to create tokio runtime for bw writing.");
        let writer = BigWigWrite::create_file(ofile, chromsizes).unwrap();
        let _ = writer.write(vals, runtime);
    }
}

pub fn read_gtffile(gtf_file: &String, gtfparse: &Gtfparse, chroms: Vec<&String>) -> (Vec<Region>, (String, u32)) {
    // At some point this zoo of String clones should be refactored. Not now though, We have a deadline.
    let mut regions: Vec<Region> = Vec::new();
    let mut names: HashMap<String, u32> = HashMap::new();
    let mut entries: u32 = 0;
    let mut txnids: Vec<String> = Vec::new();

    let gtffile = BufReader::new(File::open(gtf_file).unwrap());

    if gtfparse.metagene {
        // metagene implementation - more work here.
        let mut txn_hash: HashMap<String, Vec<(u32, u32)>> = HashMap::new();
        let mut txn_strand: HashMap<String, String> = HashMap::new();
        let mut txn_chrom: HashMap<String, String> = HashMap::new();

        for line in gtffile.lines() {
            let line = line.unwrap();
            // skip comments
            if line.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> = line.split('\t').collect();
            if fields[2].to_string() == gtfparse.exonid {
                let start = fields[3].parse().unwrap();
                let end = fields[4].parse().unwrap();
                let txnid = fields[8]
                    .split(';')
                    .find(|x| x.trim().starts_with(gtfparse.txniddesignator.as_str()))
                    .and_then(|x| x.split('"').nth(1))
                    .map(|s| s.to_string())
                    .unwrap();
                if !txnids.contains(&txnid) {
                    txnids.push(txnid.clone());
                }

                let txnentry = txn_hash.entry(txnid.clone()).or_insert(Vec::new());
                // Just to verify all exons are on the same strand.
                if txn_strand.contains_key(&txnid) {
                    assert_eq!(txn_strand.get(&txnid).unwrap(), fields[6]);
                } else {
                    txn_strand.insert(txnid.clone(), fields[6].to_string());
                }
                // Same for chromosome
                if txn_chrom.contains_key(&txnid) {
                    assert_eq!(txn_chrom.get(&txnid).unwrap(), fields[0]);
                } else {
                    txn_chrom.insert(txnid, fields[0].to_string());
                }
                txnentry.push((start, end));
            }
        }

        for txnid in txnids.into_iter() {
            
            let txnentry = txn_hash.get_mut(&txnid).unwrap();
            let length: u32 = txnentry.iter().map(|(s, e)| e - s).sum();
            txnentry.sort_by(|a, b| a.0.cmp(&b.0));
            let (starts, ends): (Vec<u32>, Vec<u32>) = txnentry.iter().map(|(s, e)| (*s, *e)).unzip();
            let chrom = txn_chrom.get(&txnid).unwrap().to_string();

            if !chroms.contains(&&chrom) {
                println!("Warning, region {} not found in at least one of the bigwig files. Skipping {}.", chrom, txnid);
            } else {
                regions.push(
                    Region {
                        chrom: txn_chrom.get(&txnid).unwrap().to_string(), //chrom
                        start: Revalue::V(starts), //start
                        end: Revalue::V(ends), //end
                        score: ".".to_string(), //score
                        strand: txn_strand.get(&txnid).unwrap().to_string(), // strand
                        name: txnid.to_string(), 
                        regionlength: length // regionlength
                    }
                );
                entries += 1;
            }
        }
    } else {
        // Take fields with col 3 == gtfparse.txnid, start, end
        for line in gtffile.lines() {
            let line = line.unwrap();
            // skip comments
            if line.starts_with('#') {
                continue;
            }

            let fields: Vec<&str> = line.split('\t').collect();
            if fields[2].to_string() == gtfparse.txnid {
                let start = fields[3].parse().unwrap();
                let end = fields[4].parse().unwrap();
                let mut entryname = fields[8]
                    .split(';')
                    .find(|x| x.trim().starts_with(gtfparse.txniddesignator.as_str()))
                    .and_then(|x| x.split('"').nth(1))
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("{}:{}-{}", fields[0], fields[1], fields[2]));
                
                if names.contains_key(&entryname) {
                    let count = names.get_mut(&entryname).unwrap();
                    *count += 1;
                    entryname = format!("{}_r{}", entryname, count);
                } else {
                    names.insert(entryname.clone(), 0);
                }

                if !chroms.contains(&&fields[0].to_string()) {
                    println!("Warning, region {} not found in at least one of the bigwig files. Skipping {}.", fields[0], entryname);
                } else {
                    regions.push(
                        Region {
                            chrom: fields[0].to_string(), //chrom
                            start: Revalue::U(start), //start
                            end: Revalue::U(end), //end
                            score: fields[5].to_string(), //score
                            strand: fields[6].to_string(), //strand
                            name: entryname, //region name
                            regionlength: end - start // regionlength
                        }
                    );
                    entries += 1;
                }
            }
        }
    }
    let filename = Path::new(gtf_file)
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .into_owned();

    return (regions, (filename, entries));
}

pub fn read_bedfile(bed_file: &String, metagene: bool, chroms: Vec<&String>) -> (Vec<Region>, (String, u32)) {
    // read a provided bed_file into a vec of Region
    // Additional return is the filename and the number of entries (for sorting later on if needed).

    let mut regions: Vec<Region> = Vec::new();
    let mut names: HashMap<String, u32> = HashMap::new();
    let mut nonbed12: bool = false;
    let mut entries: u32 = 0;

    let bedfile = BufReader::new(File::open(bed_file).unwrap());
    
    for line in bedfile.lines() {
        let line = line.unwrap();
        let fields: Vec<&str> = line.split('\t').collect();
        // Depending on bedfile, we have either BED3, BED6 or BED12
        // Note that this approach could allow somebody to have a 'mixed' bedfile, why not.
        match fields.len() {
            3 => {
                if !nonbed12 {
                    nonbed12 = true;
                }
                let chrom = fields[0];
                let mut entryname = format!("{}:{}-{}", fields[0], fields[1], fields[2]);
                if !chroms.contains(&&chrom.to_string()) {
                    println!("Warning, region {} not found in at least one of the bigwig files. Skipping {}.", chrom, entryname);
                    continue;
                }
                if names.contains_key(&entryname) {
                    let count = names.get_mut(&entryname).unwrap();
                    *count += 1;
                    entryname = format!("{}_r{}", entryname, count);
                } else {
                    names.insert(entryname.clone(), 0);
                }
                let start = fields[1].parse().unwrap();
                let end = fields[2].parse().unwrap();
                regions.push(
                    Region {
                        chrom: fields[0].to_string(), //chrom
                        start: Revalue::U(start), //start
                        end: Revalue::U(end), //end
                        score: ".".to_string(), //score
                        strand: ".".to_string(), //score
                        name: entryname, //region name
                        regionlength: end - start // regionlength
                    }
                );
                entries += 1;
            },
            6 => {
                if !nonbed12 {
                    nonbed12 = true;
                }
                let chrom = fields[0];
                let mut entryname = fields[3].to_string();
                if !chroms.contains(&&chrom.to_string()) {
                    println!("Warning, region {} not found in at least one of the bigwig files. Skipping {}.", chrom, entryname);
                    continue;
                }
                let start = fields[1].parse().unwrap();
                let end = fields[2].parse().unwrap();
                if names.contains_key(&entryname) {
                    let count = names.get_mut(&entryname).unwrap();
                    *count += 1;
                    entryname = format!("{}_r{}", entryname, count);
                } else {
                    names.insert(entryname.clone(), 0);
                }
                regions.push(
                    Region {
                        chrom: fields[0].to_string(), //chrom
                        start: Revalue::U(start), //start
                        end: Revalue::U(end), //end
                        score: ".".to_string(), //score
                        strand: ".".to_string(), //score
                        name: entryname, //region name
                        regionlength: end - start // regionlength
                    }
                );
                entries += 1;
            },
            12 => {
                let chrom = fields[0];
                let mut entryname = fields[3].to_string();
                if !chroms.contains(&&chrom.to_string()) {
                    println!("Warning, region {} not found in at least one of the bigwig files. Skipping {}.", chrom, entryname);
                    continue;
                }
                if names.contains_key(&entryname) {
                    let count = names.get_mut(&entryname).unwrap();
                    *count += 1;
                    entryname = format!("{}_r{}", entryname, count);
                } else {
                    names.insert(entryname.clone(), 0);
                }
                if metagene {                        
                    let start: u32 = fields[1].parse().unwrap();
                    let blocksizes: Vec<u32> = fields[10]
                        .split(',')
                        .filter(|x| !x.is_empty())
                        .map(|x| x.parse().unwrap())
                        .collect();
                    let length: u32 = blocksizes.iter().sum();
                    let blockstarts: Vec<u32> = fields[11]
                        .split(',')
                        .filter(|x| !x.is_empty())
                        .map(|x| x.parse::<u32>().unwrap() + start)
                        .collect();

                    let (starts, ends) = blocksizes
                        .into_iter()
                        .zip(blockstarts.into_iter())
                        .map(|(s, start)| (start, start + s))
                        .into_iter()
                        .unzip();
                    regions.push(
                        Region {
                            chrom: fields[0].to_string(), //chrom
                            start: Revalue::V(starts), //start
                            end: Revalue::V(ends), //end
                            score: fields[4].to_string(), //score
                            strand: fields[5].to_string(), //score
                            name: entryname, //region name
                            regionlength: length // regionlength
                        }
                    );
                    entries += 1;
                } else {
                    let start = fields[1].parse().unwrap();
                    let end = fields[2].parse().unwrap();
                    regions.push(
                        Region {
                            chrom: fields[0].to_string(), //chrom
                            start: Revalue::U(start), //start
                            end: Revalue::U(end), //end
                            score: fields[4].to_string(), //score
                            strand: fields[5].to_string(), //score
                            name: entryname, //region name
                            regionlength: end - start // regionlength
                        }
                    );
                    entries += 1;
                }
            },
            _ => panic!("Invalid BED format. BED file doesn't have 3, 6 or 12 fields."),
        }
    }

    let filename = Path::new(bed_file)
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .into_owned();

    if metagene && nonbed12 {
        println!("Warning: Metagene analysis is requested, but not all bedfiles and/or bedfile entries are in BED12 format. Proceed at your own risk.");
    }
    return (regions, (filename, entries));
}

pub fn chrombounds_from_bw(bwfile: &str) -> HashMap<String, u32> {
    // define chromsizes hashmap
    let mut chromsizes: HashMap<String, u32> = HashMap::new();
    let bwf = File::open(bwfile).expect("Failed to open bw file.");
    let mut reader = BigWigRead::open(bwf).unwrap();
    for chrom in reader.chroms() {
        chromsizes.insert(chrom.name.clone(), chrom.length);
    }
    chromsizes
}

pub fn bwintervals(
    bwfile: &str,
    regions: &Vec<Region>,
    slopregions: &Vec<Vec<Bin>>,
    scale_regions: &Scalingregions
) -> Vec<Vec<f32>> {
    // For a given bw file, a vector of slopregions (Bin enum))
    // return a vector with for every region a vector of f64.

    // Make sure regions and slopregions are of equal length
    assert_eq!(
        regions.len(), slopregions.len(),
        "Regions from bed file and parsed regions (slopped) do not have equal length. Something went wrong during computation."
    );
    
    // Define return vector, set up bw reader.
    let mut bwvals: Vec<Vec<f32>> = Vec::new();
    let bwf = File::open(bwfile).expect("Failed to open bw file.");
    let mut reader = BigWigRead::open(bwf).unwrap();

    // Iterate over regions and slopregions synchronously.
    for (sls, region) in slopregions.iter().zip(regions.iter()) {
        let mut bwval: Vec<f32> = Vec::new();
        // get 'min and max' to query
        // at some point this should become an impl but man am I tired.
        let (min, max) = sls
            .iter()
            .flat_map(|bin| match bin {
                Bin::Conbin(a, b) => vec![*a, *b],
                Bin::Catbin(pairs) => pairs.iter().flat_map(|(x, y)| vec![*x, *y]).collect::<Vec<u32>>(),
            })
            .fold((u32::MAX, u32::MIN), |(min, max), x| (min.min(x), max.max(x)));
        let binvals = reader.get_interval(&region.chrom, min as u32, max as u32).unwrap();
        // since binvals (can) be over binsizes, we expand them to bp and push them to a hashmap
        let mut bwhash: HashMap<u32, f32> = HashMap::new();
        for interval in binvals {
            let interval = interval.unwrap();
            let start = interval.start as u32;
            let end = interval.end as u32;
            let val = interval.value as f32;
            bwhash.extend((start..end).map(|bp| (bp, val)));
        }
        // Now we can iterate over the slopped regions, and get the values from the hashmap.
        for bin in sls {
            match bin {
                Bin::Conbin(a, b) => {
                    if a == b && *b == 0 {
                        if scale_regions.missingdata_as_zero {
                            bwval.push(0.0);
                        } else {
                            bwval.push(std::f32::NAN);
                        }
                    } else {
                        // Get values from the hashmap
                        let vals: Vec<&f32> = (*a..*b)
                            .filter_map(|bp| bwhash.get(&bp))
                            .collect();
                        let val = match scale_regions.avgtype.as_str() {
                            "mean" => mean_float(vals),
                            "median" => median_float(vals),
                            "min" => min_float(vals),
                            "max" => max_float(vals),
                            "std" => std_float(vals),
                            "sum" => sum_float(vals),
                            _ => panic!("Unknown avgtype."),
                        };
                        bwval.push(val);
                    }
                },
                Bin::Catbin(pairs) => {
                    let mut vals: Vec<&f32> = Vec::new();

                    for (start, end) in pairs {
                        if start == end && *end == 0 {
                            if scale_regions.missingdata_as_zero {
                                vals.push(&0.0);
                            } else {
                                vals.push(&std::f32::NAN);
                            }
                        } else {
                            // Get values from the hashmap
                            (*start..*end)
                                .filter_map(|bp| bwhash.get(&bp))
                                .for_each(|v| vals.push(v));
                        }
                    }

                    let val = match scale_regions.avgtype.as_str() {
                        "mean" => mean_float(vals),
                        "median" => median_float(vals),
                        "min" => min_float(vals),
                        "max" => max_float(vals),
                        "std" => std_float(vals),
                        "sum" => sum_float(vals),
                        _ => panic!("Unknown avgtype."),
                    };
                    bwval.push(val);
                }
            }
        }
        assert_eq!(bwval.len(), scale_regions.cols_expected / scale_regions.bwfiles);
        bwvals.push(bwval);
    }
    bwvals
}

pub fn header_matrix(scale_regions: &Scalingregions, regionsizes: HashMap<String, u32>) -> String {
    // Create the header for the matrix.
    // This is quite ugly, but this is mainly because we need to accomodate delta bwfiles.
    let mut headstr = String::new();
    headstr.push_str("@{");
    headstr.push_str(
        &format!("\"upstream\":[{}],", (0..scale_regions.bwfiles).map(|_| scale_regions.upstream).collect::<Vec<_>>().into_iter().join(","))
    );
    headstr.push_str(
        &format!("\"downstream\":[{}],", (0..scale_regions.bwfiles).map(|_| scale_regions.downstream).collect::<Vec<_>>().into_iter().join(","))
    );
    headstr.push_str(
        &format!("\"body\":[{}],", (0..scale_regions.bwfiles).map(|_| scale_regions.regionbodylength).collect::<Vec<_>>().into_iter().join(","))
    );
    headstr.push_str(
        &format!("\"bin size\":[{}],", (0..scale_regions.bwfiles).map(|_| scale_regions.binsize).collect::<Vec<_>>().into_iter().join(","))
    );
    headstr.push_str(
        &format!("\"ref point\":[\"{}\"],", (0..scale_regions.bwfiles).map(|_| scale_regions.referencepoint.clone()).collect::<Vec<_>>().into_iter().join("\",\""))
    );
    headstr.push_str(
        &format!("\"verbose\":{},", scale_regions.verbose)
    );
    headstr.push_str(
        &format!("\"bin avg type\":\"{}\",", scale_regions.avgtype)
    );
    headstr.push_str(
        &format!("\"missing data as zero\":{},", scale_regions.missingdata_as_zero)
    );
    // Unimplemented arguments, but they need to be present in header for now anyway.
    headstr.push_str(
        "\"min threshold\":null,\"max threshold\":null,\"scale\":1,\"skip zeros\":false,\"nan after end\":false,"
    );
    headstr.push_str(
        &format!("\"proc number\":{},", scale_regions.proc_number)
    );
    // Unimplemented for now...
    headstr.push_str(
        "\"sort regions\":\"keep\",\"sort using\":\"mean\","
    );
    headstr.push_str(
        &format!("\"unscaled 5 prime\":[{}],", (0..scale_regions.bwfiles).map(|_| scale_regions.unscaled5prime).collect::<Vec<_>>().into_iter().join(","))
    );
    headstr.push_str(
        &format!("\"unscaled 3 prime\":[{}],", (0..scale_regions.bwfiles).map(|_| scale_regions.unscaled3prime).collect::<Vec<_>>().into_iter().join(","))
    );
    headstr.push_str(
        &format!("\"group_labels\":[\"{}\"],", scale_regions.regionlabels.join("\",\""))
    );
    // Get cumulative sizes of regions
    let mut groupbounds: Vec<u32> = Vec::new();
    groupbounds.push(0);
    let mut cumsum: u32 = 0;
    for regionlabel in scale_regions.regionlabels.iter() {
        cumsum += regionsizes.get(regionlabel).unwrap();
        groupbounds.push(cumsum);
    }
    let groupbounds = format!("{}", groupbounds.iter()
        .map(|&x| x.to_string())
        .collect::<Vec<String>>()
        .join(","));

    headstr.push_str(
        &format!("\"group_boundaries\":[{}],", groupbounds)
    );
    // Sample labels
    headstr.push_str(
        &format!("\"sample_labels\":[\"{}\"],", scale_regions.bwlabels.join("\",\""))
    );
    // Get cumulative sizes for sample boundaries
    let colsize_per_sample = scale_regions.cols_expected / scale_regions.bwfiles;
    let mut samplebounds: Vec<u64> = Vec::new();
    samplebounds.push(0);
    let mut cumsum: u64 = 0;
    for i in 0..scale_regions.bwfiles {
        cumsum += colsize_per_sample as u64;
        samplebounds.push(cumsum);
    }
    let samplebounds = format!("{}", samplebounds.iter()
        .map(|&x| x.to_string())
        .collect::<Vec<String>>()
        .join(","));

    headstr.push_str(
        &format!("\"sample_boundaries\":[{}]", samplebounds)
    );
    headstr.push_str(
        "}\n"
    );
    headstr
}

pub fn write_matrix(
    header: String,
    mat: Vec<Vec<f32>>,
    ofile: &str,
    regions: Vec<Region>,
    scale_regions: &Scalingregions
) {
    // Write out the matrix to a compressed file.
    let omat = File::create(ofile).unwrap();
    let mut encoder = GzEncoder::new(omat, Compression::default());
    encoder.write_all(header.as_bytes()).unwrap();
    // Final check to make sure our regions and mat iter are of equal length.
    assert_eq!(regions.len(), mat.len());
    for (region, row) in regions.into_iter().zip(mat.into_iter()) {
        // Skipping rules.
        // skip_zeros
        if scale_regions.skipzero && row.iter().all(|&x| x == 0.0) {
            continue;
        }
        // min threshold
        if scale_regions.minthresh != 0.0 && row.iter().any(|&x| x < scale_regions.minthresh) {
            continue;
        }
        // max threshold
        if scale_regions.maxthresh != 0.0 && row.iter().any(|&x| x > scale_regions.maxthresh) {
            continue;
        }
        let mut writerow = format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t",
            region.chrom,                // Chromosome
            region.start.rewrites(),     // Revalue for start (either u32, or Vec<u32>)
            region.end.rewrites(),       // Revalue for end (either u32, or Vec<u32>)
            region.name,                 // String field for name. Duplicates taken care of in read_bedfiles.
            region.score,                // Score field persisted from bedfile
            region.strand,               // Strand field persisted from bedfile
        );
        writerow.push_str(
            &row.iter().map(|x| (scale_regions.scale * x).to_string()).collect::<Vec<String>>().join("\t")
        );
        writerow.push_str("\n");
        encoder.write_all(writerow.as_bytes()).unwrap();
    }
}
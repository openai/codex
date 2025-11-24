#![cfg_attr(specialize, feature(build_hasher_simple_hash_one))]

use std::hash::{BuildHasher, Hash, Hasher};

use ahash::RandomState;
use criterion::*;
use fxhash::FxHasher;

fn gen_word_pairs() -> Vec<String> {
    let words: Vec<_> = r#"
a, ability, able, about, above, accept, according, account, across, act, action,
activity, actually, add, address, administration, admit, adult, affect, after,
again, against, age, agency, agent, ago, agree, agreement, ahead, air, all,
allow, almost, alone, along, already, also, although, always, American, among,
amount, analysis, and, animal, another, answer, any, anyone, anything, appear,
apply, approach, area, argue, arm, around, arrive, art, article, artist, as,
ask, assume, at, attack, attention, attorney, audience, author, authority,
available, avoid, away, baby, back, bad, bag, ball, bank, bar, base, be, beat,
beautiful, because, become, bed, before, begin, behavior, behind, believe,
benefit, best, better, between, beyond, big, bill, billion, bit, black, blood,
blue, board, body, book, born, both, box, boy, break, bring, brother, budget,
build, building, business, but, buy, by, call, camera, campaign, can, cancer,
candidate, capital, car, card, care, career, carry, case, catch, cause, cell,
center, central, century, certain, certainly, chair, challenge, chance, change,
character, charge, check, child, choice, choose, church, citizen, city, civil,
claim, class, clear, clearly, close, coach, cold, collection, college, color,
come, commercial, common, community, company, compare, computer, concern,
condition, conference, Congress, consider, consumer, contain, continue, control,
cost, could, country, couple, course, court, cover, create, crime, cultural,
culture, cup, current, customer, cut, dark, data, daughter, day, dead, deal,
death, debate, decade, decide, decision, deep, defense, degree, Democrat,
democratic, describe, design, despite, detail, determine, develop, development,
die, difference, different, difficult, dinner, direction, director, discover,
discuss, discussion, disease, do, doctor, dog, door, down, draw, dream, drive,
drop, drug, during, each, early, east, easy, eat, economic, economy, edge,
education, effect, effort, eight, either, election, else, employee, end, energy,
enjoy, enough, enter, entire, environment, environmental, especially, establish,
even, evening, event, ever, every, everybody, everyone, everything, evidence,
exactly, example, executive, exist, expect, experience, expert, explain, eye,
face, fact, factor, fail, fall, family, far, fast, father, fear, federal, feel,
feeling, few, field, fight, figure, fill, film, final, finally, financial, find,
fine, finger, finish, fire, firm, first, fish, five, floor, fly, focus, follow,
food, foot, for, force, foreign, forget, form, former, forward, four, free,
friend, from, front, full, fund, future, game, garden, gas, general, generation,
get, girl, give, glass, go, goal, good, government, great, green, ground, group,
grow, growth, guess, gun, guy, hair, half, hand, hang, happen, happy, hard,
have, he, head, health, hear, heart, heat, heavy, help, her, here, herself,
high, him, himself, his, history, hit, hold, home, hope, hospital, hot, hotel,
hour, house, how, however, huge, human, hundred, husband, I, idea, identify, if,
image, imagine, impact, important, improve, in, include, including, increase,
indeed, indicate, individual, industry, information, inside, instead,
institution, interest, interesting, international, interview, into, investment,
involve, issue, it, item, its, itself, job, join, just, keep, key, kid, kill,
kind, kitchen, know, knowledge, land, language, large, last, late, later, laugh,
law, lawyer, lay, lead, leader, learn, least, leave, left, leg, legal, less,
let, letter, level, lie, life, light, like, likely, line, list, listen, little,
live, local, long, look, lose, loss, lot, love, low, machine, magazine, main,
maintain, major, majority, make, man, manage, management, manager, many, market,
marriage, material, matter, may, maybe, me, mean, measure, media, medical, meet,
meeting, member, memory, mention, message, method, middle, might, military,
million, mind, minute, miss, mission, model, modern, moment, money, month, more,
morning, most, mother, mouth, move, movement, movie, Mr, Mrs, much, music, must,
my, myself, name, nation, national, natural, nature, near, nearly, necessary,
need, network, never, new, news, newspaper, next, nice, night, no, none, nor,
north, not, note, nothing, notice, now, n't, number, occur, of, off, offer,
office, officer, official, often, oh, oil, ok, old, on, once, one, only, onto,
open, operation, opportunity, option, or, order, organization, other, others,
our, out, outside, over, own, owner, page, pain, painting, paper, parent, part,
participant, particular, particularly, partner, party, pass, past, patient,
pattern, pay, peace, people, per, perform, performance, perhaps, period, person,
personal, phone, physical, pick, picture, piece, place, plan, plant, play,
player, PM, point, police, policy, political, politics, poor, popular,
population, position, positive, possible, power, practice, prepare, present,
president, pressure, pretty, prevent, price, private, probably, problem,
process, produce, product, production, professional, professor, program,
project, property, protect, prove, provide, public, pull, purpose, push, put,
quality, question, quickly, quite, race, radio, raise, range, rate, rather,
reach, read, ready, real, reality, realize, really, reason, receive, recent,
recently, recognize, record, red, reduce, reflect, region, relate, relationship,
religious, remain, remember, remove, report, represent, Republican, require,
research, resource, respond, response, responsibility, rest, result, return,
reveal, rich, right, rise, risk, road, rock, role, room, rule, run, safe, same,
save, say, scene, school, science, scientist, score, sea, season, seat, second,
section, security, see, seek, seem, sell, send, senior, sense, series, serious,
serve, service, set, seven, several, sex, sexual, shake, share, she, shoot,
short, shot, should, shoulder, show, side, sign, significant, similar, simple,
simply, since, sing, single, sister, sit, site, situation, six, size, skill,
skin, small, smile, so, social, society, soldier, some, somebody, someone,
something, sometimes, son, song, soon, sort, sound, source, south, southern,
space, speak, special, specific, speech, spend, sport, spring, staff, stage,
stand, standard, star, start, state, statement, station, stay, step, still,
stock, stop, store, story, strategy, street, strong, structure, student, study,
stuff, style, subject, success, successful, such, suddenly, suffer, suggest,
summer, support, sure, surface, system, table, take, talk, task, tax, teach,
teacher, team, technology, television, tell, ten, tend, term, test, than, thank,
that, the, their, them, themselves, then, theory, there, these, they, thing,
think, third, this, those, though, thought, thousand, threat, three, through,
throughout, throw, thus, time, to, today, together, tonight, too, top, total,
tough, toward, town, trade, traditional, training, travel, treat, treatment,
tree, trial, trip, trouble, true, truth, try, turn, TV, two, type, under,
understand, unit, until, up, upon, us, use, usually, value, various, very,
victim, view, violence, visit, voice, vote, wait, walk, wall, want, war, watch,
water, way, we, weapon, wear, week, weight, well, west, western, what, whatever,
when, where, whether, which, while, white, who, whole, whom, whose, why, wide,
wife, will, win, wind, window, wish, with, within, without, woman, wonder, word,
work, worker, world, worry, would, write, writer, wrong, yard, yeah, year, yes,
yet, you, young, your, yourself"#
        .split(',')
        .map(|word| word.trim())
        .collect();

    let mut word_pairs: Vec<_> = Vec::new();
    for word in &words {
        for other_word in &words {
            word_pairs.push(word.to_string() + " " + other_word);
        }
    }
    assert_eq!(1_000_000, word_pairs.len());
    word_pairs
}

#[allow(unused)] // False positive
fn test_hash_common_words<B: BuildHasher>(build_hasher: &B) {
    let word_pairs: Vec<_> = gen_word_pairs();
    check_for_collisions(build_hasher, &word_pairs, 32);
}

#[allow(unused)] // False positive
fn check_for_collisions<H: Hash, B: BuildHasher>(build_hasher: &B, items: &[H], bucket_count: usize) {
    let mut buckets = vec![0; bucket_count];
    for item in items {
        let value = hash(item, build_hasher) as usize;
        buckets[value % bucket_count] += 1;
    }
    let mean = items.len() / bucket_count;
    let max = *buckets.iter().max().unwrap();
    let min = *buckets.iter().min().unwrap();
    assert!(
        (min as f64) > (mean as f64) * 0.95,
        "min: {}, max:{}, {:?}",
        min,
        max,
        buckets
    );
    assert!(
        (max as f64) < (mean as f64) * 1.05,
        "min: {}, max:{}, {:?}",
        min,
        max,
        buckets
    );
}

#[cfg(specialize)]
#[allow(unused)] // False positive
fn hash<H: Hash, B: BuildHasher>(b: &H, build_hasher: &B) -> u64 {
    build_hasher.hash_one(b)
}

#[cfg(not(specialize))]
#[allow(unused)] // False positive
fn hash<H: Hash, B: BuildHasher>(b: &H, build_hasher: &B) -> u64 {
    let mut hasher = build_hasher.build_hasher();
    b.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn test_bucket_distribution() {
    let build_hasher = RandomState::with_seeds(1, 2, 3, 4);
    test_hash_common_words(&build_hasher);
    let sequence: Vec<_> = (0..320000).collect();
    check_for_collisions(&build_hasher, &sequence, 32);
    let sequence: Vec<_> = (0..2560000).collect();
    check_for_collisions(&build_hasher, &sequence, 256);
    let sequence: Vec<_> = (0..320000).map(|i| i * 1024).collect();
    check_for_collisions(&build_hasher, &sequence, 32);
    let sequence: Vec<_> = (0..2560000_u64).map(|i| i * 1024).collect();
    check_for_collisions(&build_hasher, &sequence, 256);
}

#[cfg(feature = "std")]
#[test]
fn test_ahash_alias_map_construction() {
    let mut map = ahash::HashMap::default();
    map.insert(1, "test");
    use ahash::HashMapExt;
    let mut map = ahash::HashMap::with_capacity(1234);
    map.insert(1, "test");
}

#[cfg(feature = "std")]
#[test]
fn test_ahash_alias_set_construction() {
    let mut set = ahash::HashSet::default();
    set.insert(1);

    use ahash::HashSetExt;
    let mut set = ahash::HashSet::with_capacity(1235);
    set.insert(1);
}


#[cfg(feature = "std")]
#[test]
fn test_key_ref() {
    let mut map = ahash::HashMap::default();
    map.insert(1, "test");
    assert_eq!(Some((1, "test")), map.remove_entry(&1));

    let mut map = ahash::HashMap::default();
    map.insert(&1, "test");
    assert_eq!(Some((&1, "test")), map.remove_entry(&&1));

    let mut m = ahash::HashSet::<Box<String>>::default();
    m.insert(Box::from("hello".to_string()));
    assert!(m.contains(&"hello".to_string()));

    let mut m = ahash::HashSet::<String>::default();
    m.insert("hello".to_string());
    assert!(m.contains("hello"));

    let mut m = ahash::HashSet::<Box<[u8]>>::default();
    m.insert(Box::from(&b"hello"[..]));
    assert!(m.contains(&b"hello"[..]));
}

#[cfg(feature = "std")]
#[test]
fn test_byte_dist() {
    use rand::{SeedableRng, Rng, RngCore};
    use pcg_mwc::Mwc256XXA64;

    let mut r = Mwc256XXA64::seed_from_u64(0xe786_c22b_119c_1479);
    let mut lowest = 2.541;
    let mut highest = 2.541;
    for _round in 0..100 {
        let mut table: [bool; 256 * 8] = [false; 256 * 8];
        let hasher = RandomState::with_seeds(r.gen(), r.gen(), r.gen(), r.gen());
        for i in 0..128 {
            let mut keys: [u8; 8] = hasher.hash_one((i as u64) << 30).to_ne_bytes();
            //let mut keys = r.next_u64().to_ne_bytes(); //This is a control to test assert sensitivity.
            for idx in 0..8 {
                while table[idx * 256 + keys[idx] as usize] {
                    keys[idx] = keys[idx].wrapping_add(1);
                }
                table[idx * 256 + keys[idx] as usize] = true;
            }
        }

        for idx in 0..8 {
            let mut len = 0;
            let mut total_len = 0;
            let mut num_seq = 0;
            for i in 0..256 {
                if table[idx * 256 + i] {
                    len += 1;
                } else if len != 0 {
                    num_seq += 1;
                    total_len += len;
                    len = 0;
                }
            }
            let mean = total_len as f32 / num_seq as f32;
            println!("Mean sequence length = {}", mean);
            if mean > highest {
                highest = mean;
            }
            if mean < lowest {
                lowest = mean;
            }
        }
    }
    assert!(lowest > 1.9, "Lowest = {}", lowest);
    assert!(highest < 3.9, "Highest = {}", highest);
}


fn ahash_vec<H: Hash>(b: &Vec<H>) -> u64 {
    let mut total: u64 = 0;
    for item in b {
        let mut hasher = RandomState::with_seeds(12, 34, 56, 78).build_hasher();
        item.hash(&mut hasher);
        total = total.wrapping_add(hasher.finish());
    }
    total
}

fn fxhash_vec<H: Hash>(b: &Vec<H>) -> u64 {
    let mut total: u64 = 0;
    for item in b {
        let mut hasher = FxHasher::default();
        item.hash(&mut hasher);
        total = total.wrapping_add(hasher.finish());
    }
    total
}

fn bench_ahash_words(c: &mut Criterion) {
    let words = gen_word_pairs();
    c.bench_function("aes_words", |b| b.iter(|| black_box(ahash_vec(&words))));
}

fn bench_fx_words(c: &mut Criterion) {
    let words = gen_word_pairs();
    c.bench_function("fx_words", |b| b.iter(|| black_box(fxhash_vec(&words))));
}

criterion_main!(benches);
criterion_group!(benches, bench_ahash_words, bench_fx_words,);

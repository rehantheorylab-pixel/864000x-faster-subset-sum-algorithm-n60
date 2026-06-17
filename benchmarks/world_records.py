#!/usr/bin/env python3
"""World Record Benchmark Suite for Z++ Subset Sum Solver."""
import json, time, urllib.request, sys, os, random, math

PORT = 8080
if "--port" in sys.argv:
    PORT = int(sys.argv[sys.argv.index("--port") + 1])
random.seed(42)

def solve(numbers, target, timeout=30):
    req = json.dumps({"numbers": numbers, "target": str(target), "timeout": timeout}).encode()
    try:
        r = urllib.request.urlopen(
            urllib.request.Request(f"http://127.0.0.1:{PORT}/api/solve",
                data=req, headers={"Content-Type": "application/json"}),
            timeout=timeout + 10)
        return json.loads(r.read())
    except:
        return {"result": "error", "error": "timeout", "time_ns": 0, "winner": "N/A", "solution": ""}

def gen_known_solution(n, bitlen=64, subset_frac=0.4):
    vals = [random.randint(2**(bitlen-1), 2**bitlen - 1) for _ in range(n)]
    k = max(2, int(n * subset_frac))
    subset = random.sample(range(n), k)
    target = sum(vals[i] for i in subset)
    return ",".join(str(v) for v in vals), target

def gen_all_sum(n, max_val=100):
    vals = [random.randint(1, max_val) for _ in range(n)]
    return ",".join(str(v) for v in vals), sum(vals)

def gen_super_increasing(n):
    vals = [1]
    for _ in range(n-1):
        vals.append(sum(vals) * 2 + random.randint(0, 5))
    k = random.randint(2, n)
    idxs = sorted(random.sample(range(n), k))
    return ",".join(str(v) for v in vals), sum(vals[i] for i in idxs)

def gen_powers(n):
    vals = [2**i for i in range(n)]
    return ",".join(str(v) for v in vals), sum(vals)

def gen_arith(n, step=7):
    vals = [100 + i*step for i in range(n)]
    k = random.randint(2, n)
    idxs = sorted(random.sample(range(n), k))
    return ",".join(str(v) for v in vals), sum(vals[i] for i in idxs)

def run(name, numbers, target, timeout=60):
    t0 = time.time()
    r = solve(numbers, target, timeout)
    t1 = time.time()
    nel = len([x for x in numbers.split(",") if x.strip()]) if numbers.strip() else 0
    md = max((len(x.strip().lstrip("-")) for x in numbers.split(",") if x.strip()), default=0)
    return {"name": name, "n": nel, "digits": md,
            "result": r.get("result","error"), "winner": r.get("winner","N/A"),
            "time_ms": (t1-t0)*1000, "server_ms": r.get("time_ns",0)/1e6}

SPECS = "Intel i3-2100 @ 3.10GHz (2C/4T) | 12GB DDR3 | Win10 Pro | Rust 1.95 Debug"

def main():
    R = []
    print("="*60)
    print(f"  Z++ BENCHMARKS  |  {SPECS}")
    print("="*60)

    # EDGE CASES
    R.append(run("Empty set", "", "0", 5))
    R.append(run("Single match", "7", "7", 5))
    R.append(run("Single impossible", "7", "5", 5))
    R.append(run("Two-elem match", "3,8", "11", 5))
    R.append(run("Target=0, 10 elems", "1,2,3,4,5,6,7,8,9,10", "0", 5))
    R.append(run("30x duplicates", "7,"*29+"7", "49", 5))
    R.append(run("Has zero element", "0,1,2,3,4,5", "7", 5))
    R.append(run("Negative values", "-5,3,8,-2,7,-1,4,9,-3,6", "15", 10))

    # IMPOSSIBLE
    R.append(run("GCD impossible", "3,6,9,12,15,18,21,24", "10", 5))
    R.append(run("Even/odd mismatch", "2,4,6,8,10,12,14,16", "7", 5))
    R.append(run("Sum < target", "1,2,3,4,5", "100", 5))

    # ALL-ELEMENTS
    n,t = gen_all_sum(10,50); R.append(run("All elems n=10", n, t, 5))
    n,t = gen_all_sum(50,100); R.append(run("All elems n=50", n, t, 5))
    n,t = gen_all_sum(100,200); R.append(run("All elems n=100", n, t, 5))

    # SUPER-INCREASING
    for sz in [20, 40, 60]:
        n,t = gen_super_increasing(sz); R.append(run(f"Super-inc n={sz}", n, t, 5))

    # POWERS OF 2
    n,t = gen_powers(10); R.append(run("Pow2 n=10", n, t, 5))
    n,t = gen_powers(20); R.append(run("Pow2 n=20", n, t, 5))

    # BITSET DP
    for sz in [100, 500, 1000, 2000]:
        vals = [random.randint(1,100) for _ in range(sz)]
        k = random.randint(2, min(20, sz))
        target = sum(vals[i] for i in sorted(random.sample(range(sz), k)))
        R.append(run(f"BitsetDP n={sz}", ",".join(str(v) for v in vals), target, 30))

    # RANDOM / MITM
    for n,b in [(20,40),(25,48),(30,56)]:
        s,t = gen_known_solution(n,b,0.3); R.append(run(f"Random n={n} {b}b", s, t, 60))

    # DENSE
    for n in [20,30,40]:
        vals = [random.randint(1,max(5,100-n)) for _ in range(n)]
        k = random.randint(2, min(15,n))
        target = sum(vals[i] for i in sorted(random.sample(range(n), k)))
        R.append(run(f"Dense n={n}", ",".join(str(v) for v in vals), target, 30))

    # HARD 64-BIT
    for n in [36,40,44]:
        s,t = gen_known_solution(n, 64, 0.35); R.append(run(f"Hard 64b n={n}", s, t, 180))

    # ARBITRARY PRECISION
    for n,b in [(28,90),(32,110),(36,130)]:
        s,t = gen_known_solution(n, b, 0.3); R.append(run(f"BigInt n={n} {b}b", s, t, 300))

    # CLASSICS
    R.append(run("5570 benchmark", "1,3,7,21,50,200,400,499,1000,1500,2000,5000,10000,25000", "5570", 10))
    R.append(run("Fibonacci n=20", "1,2,3,5,8,13,21,34,55,89,144,233,377,610,987,1597,2584,4181,6765,10946", "17710", 5))
    n,t = gen_arith(30); R.append(run("Arithmetic n=30", n, t, 10))
    n,t = gen_arith(50,13); R.append(run("Arithmetic n=50", n, t, 30))

    # UNIQUE SOLUTION
    for sz in [20, 30]:
        vals = [10**9 + i for i in range(1, sz+1)]
        k = random.randint(2, sz)
        target = sum(vals[i] for i in sorted(random.sample(range(sz), k)))
        R.append(run(f"Unique n={sz}", ",".join(str(v) for v in vals), target, 30))

    # VALUE SPREAD
    vals = [1,10,100,1000,10000,100000,1000000,10000000,100000000,1000000000,
            2,20,200,2000,20000,200000,2000000,20000000,200000000,2000000000]
    k=5; target=sum(vals[i] for i in sorted(random.sample(range(len(vals)),k)))
    R.append(run("Value spread n=20", ",".join(str(v) for v in vals), target, 30))

    # FREQUENCY / PATTERNED
    vals = [3]*10 + [7]*10 + [11]*10 + [13]*10
    target = 3*3 + 7*2 + 11*1 + 13*2
    R.append(run("Frequency pattern", ",".join(str(v) for v in vals), target, 10))

    # Print table
    passed = sum(1 for r in R if r["result"] in ("solved","impossible"))
    print(f"\nPASSED: {passed}/{len(R)}\n")
    print(f"PC: {SPECS}\n")
    print("| # | Category | n | Digits | Result | Engine | ms |")
    print("|---|----------|---|--------|--------|--------|-----|")
    for i,r in enumerate(R,1):
        print(f"| {i} | {r['name']} | {r['n']} | {r['digits']} | {r['result']} | {r['winner']} | {r['time_ms']:.1f} |")

    with open(os.path.join(os.path.dirname(__file__), "results.json"), "w") as f:
        json.dump({"specs": SPECS, "results": R}, f, indent=2)
    print("\nSaved.")

if __name__ == "__main__":
    main()

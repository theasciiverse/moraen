# Drawing aid for the first world's last group, the stone trolls: run it to rewrite
# caves/21 to 25. The text files are what the game loads.
import os, random
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'caves')

class C:
    def __init__(s, w=40, h=22):
        s.W, s.H = w, h
        s.g = [['.'] * w for _ in range(h)]
        for x in range(w): s.g[0][x] = s.g[h-1][x] = 'W'
        for y in range(h): s.g[y][0] = s.g[y][w-1] = 'W'
    def p(s, x, y, c):
        assert 0 < x < s.W-1 and 0 < y < s.H-1 or c in 'WX', (x, y, c)
        s.g[y][x] = c
    def h(s, x, y, n, c):
        for i in range(n): s.p(x+i, y, c)
    def v(s, x, y, n, c):
        for i in range(n): s.p(x, y+i, c)
    def rect(s, x, y, w, h, c):
        for j in range(h): s.h(x, y+j, w, c)
    def nook(s, x, y):
        s.rect(x-1, y-1, 3, 3, ':'); s.p(x, y, 'P')
    def kettle(s, x, y, w, h, fly):
        # a pocket of space ringed by dirt that never takes the fill; the fly at its top left
        s.rect(x-1, y-1, w+2, h+2, ':'); s.rect(x, y, w, h, ' '); s.p(x, y, fly)
    def perched(s, x, y, floor='w'):
        # a troll caged on a ledge, open space over it, then dirt, then a stone (dirt
        # that never takes the fill). Dig that dirt from the side: three steps from the
        # troll, so it stays asleep, and the stone comes down and kills it ("OOF!"),
        # leaving a plain boulder. On a dirt floor, dig under that boulder next.
        s.p(x, y+1, floor); s.p(x, y, 't'); s.p(x, y-1, ' '); s.p(x, y-2, ':'); s.p(x, y-3, 'r'); s.p(x, y-4, ':')
        for dx in (-1, 1):
            s.p(x+dx, y, ':'); s.p(x+dx, y-1, ':')
    def save(s, name, file, **kw):
        head = [f'name: {name}', f"about: {kw['about']}", f"value: {kw['value']}",
                f"quota: {kw['quota']}", f"time: {kw['time']}", f"seed: {kw['seed']}"]
        if kw.get('fill'): head.append(f"fill: {kw['fill']}")
        for k in ('magic', 'amoeba', 'trolls'):
            if k in kw: head.append(f'{k}: {kw[k]}')
        if s.W == 20: head.append('intermission: yes')
        rows = [''.join(r) for r in s.g]
        assert all(len(r) == s.W for r in rows)
        open(os.path.join(OUT, file), 'w').write('\n'.join(head) + '\ngrid:\n' + '\n'.join(rows) + '\n')

def seeds(n):
    r = random.Random(n)
    return ' '.join(str(r.randrange(256)) for _ in range(5))

# ---- 17-20 and an intermission: the stone trolls -------------------------------

c = C()  # trolls alone: some of these stones are asleep
c.nook(2, 2)
for x, y, n in [(7, 6, 12), (24, 5, 12), (4, 11, 13), (21, 12, 15), (9, 17, 12), (26, 17, 10)]:
    c.h(x, y, n, 'w')
    for i in range(n):
        c.p(x+i, y-1, 't' if i % 4 == 1 else 'r')
    for i in range(0, n, 3):
        c.p(x+i, y+1, 'd')
c.p(37, 20, 'X')
c.save('Talus', '21-talus.txt', about='some of these stones are only sleeping; never dig beside one',
       value='15 25', quota='12 13 14 15 16', time='150 140 130 120 110', seed=seeds(21),
       fill='boulder 30, diamond 12', trolls=50)

c = C()  # crushing: a stone over every troll on the ridges
c.nook(2, 2)
for y in (7, 13, 19):
    for x in range(7, 36, 5):
        c.perched(x, y - 1)
        c.p(x + 2, y - 1, 'd')
c.p(38, 10, 'X')
c.save('Arete', '22-arete.txt', about='a stone hangs over every caged troll; let it fall before they wake',
       value='15 25', quota='14 15 16 17 18', time='150 140 130 120 110', seed=seeds(22),
       fill='boulder 24, diamond 10', trolls=60)

c = C()  # trolls over the butterflies' pools: their rubble falls in and makes gems
c.nook(2, 2)
for x in (8, 18, 28):
    c.kettle(x, 13, 4, 3, 'b')
    # a caged troll on the pool's roof, a stone hanging over it
    c.perched(x+1, 11, ':')
c.h(4, 18, 32, 'w')
c.p(37, 20, 'X')
c.save('Tarn', '23-tarn.txt', about='crush the trolls over the pools, then drop what is left of them on the moths',
       value='10 20', quota='18 20 22 24 26', time='160 150 140 130 120', seed=seeds(23),
       fill='boulder 26, diamond 6', trolls=60)

c = C()  # trolls over the rune-stone: crushed, their rubble falls through as gems
c.nook(2, 2)
c.h(6, 14, 28, 'm')
c.rect(6, 15, 28, 4, ' ')
c.h(5, 19, 30, 'w'); c.v(5, 14, 5, 'w'); c.v(34, 14, 5, 'w')
c.p(34, 17, ':')  # the way into the basin
for x in range(8, 33, 6):
    c.perched(x, 12, ':')
for x in range(10, 33, 6):
    c.kettle(x, 8, 2, 1, 'f')
c.p(38, 20, 'X')
c.save('Bergschrund', '24-bergschrund.txt', about='the old rune-stone turns even a crushed troll into gems',
       value='10 20', quota='16 18 20 22 24', time='170 160 150 140 130', seed=seeds(24),
       fill='boulder 40, diamond 6', magic=30, trolls=70)

c = C(20, 12)  # intermission: the thaw comes early
c.nook(2, 2)
for y in (5, 8):
    c.h(5, y, 12, 'w')
    for i in range(12):
        c.p(5+i, y-1, 'd' if i % 2 == 0 else ('t' if i % 4 == 1 else 'r'))
for x in range(6, 17, 2):
    c.p(x, 10, 'd')
c.p(18, 10, 'X')
c.save('Ablation', '25-ablation.txt', about='intermission: the sun is up early, and so are the trolls',
       value='20 30', quota='8 9 10 11 12', time='30 26 24 22 20', seed=seeds(25), trolls=15)

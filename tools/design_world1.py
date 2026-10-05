# Drawing aid for Moraen's first world: run it to rewrite caves/01 to 18 and 20.
# The text files are what the game loads; Moraine (19) is hand-edited text only.
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
    def put(s, x, y, txt):
        for i, c in enumerate(txt):
            if c != '_': s.p(x+i, y, c)
    def h(s, x, y, n, c):
        for i in range(n): s.p(x+i, y, c)
    def v(s, x, y, n, c):
        for i in range(n): s.p(x, y+i, c)
    def rect(s, x, y, w, h, c):
        for j in range(h): s.h(x, y+j, w, c)
    def box(s, x, y, w, h, c):
        s.h(x, y, w, c); s.h(x, y+h-1, w, c); s.v(x, y, h, c); s.v(x+w-1, y, h, c)
    def nook(s, x, y):
        s.rect(x-1, y-1, 3, 3, ':'); s.p(x, y, 'P')
    def ring(s, x, y, w, h, wall='w'):
        # a closed loop corridor round a solid core: outer wall, 1-wide corridor, core
        s.rect(x, y, w, h, wall); s.rect(x+1, y+1, w-2, h-2, ' '); s.rect(x+2, y+2, w-4, h-4, wall)
    def kettle(s, x, y, w, h, fly):
        # a pocket of space ringed by dirt that never takes the fill; the fly at its top left
        s.rect(x-1, y-1, w+2, h+2, ':'); s.rect(x, y, w, h, ' '); s.p(x, y, fly)
    def save(s, name, file, **kw):
        head = [f'name: {name}', f"about: {kw['about']}", f"value: {kw['value']}",
                f"quota: {kw['quota']}", f"time: {kw['time']}", f"seed: {kw['seed']}"]
        if kw.get('fill'): head.append(f"fill: {kw['fill']}")
        for k in ('magic', 'amoeba'):
            if k in kw: head.append(f'{k}: {kw[k]}')
        if s.W == 20: head.append('intermission: yes')
        rows = [''.join(r) for r in s.g]
        assert all(len(r) == s.W for r in rows)
        open(os.path.join(OUT, file), 'w').write('\n'.join(head) + '\ngrid:\n' + '\n'.join(rows) + '\n')

def seeds(n):
    r = random.Random(n)
    return ' '.join(str(r.randrange(256)) for _ in range(5))

# ---- 1-4: the basics ----------------------------------------------------------

c = C()  # dig and collect
c.nook(2, 2)
c.h(8, 7, 6, 'w'); c.h(21, 12, 7, 'w'); c.h(28, 5, 6, 'w'); c.h(9, 16, 8, 'w'); c.h(30, 17, 5, 'w')
c.p(37, 19, 'X')
c.save('First Thaw', '01-first-thaw.txt', about='the ice has gone; dig, gather, get out',
       value='10 15', quota='10 12 14 16 18', time='150 140 130 120 110', seed=seeds(1),
       fill='boulder 34, diamond 12')

c = C()  # falling and rolling
c.nook(2, 2)
for x, y, n in [(5, 6, 9), (18, 5, 10), (31, 7, 6), (8, 11, 10), (24, 12, 11), (4, 16, 8), (16, 17, 9), (29, 17, 8)]:
    c.h(x, y, n, 'w')
    c.h(x+1, y-1, n-2, 'r')
    c.h(x+2, y-2, n-4, 'r')
    for i in range(1, n-1, 2):
        c.p(x+i, y+1, 'd')
c.p(37, 20, 'X')
c.save('Erratic', '02-erratic.txt', about='the ice dropped its stones wherever it liked',
       value='10 20', quota='12 14 16 18 20', time='150 140 130 120 110', seed=seeds(2),
       fill='boulder 70, diamond 8')

c = C()  # pushing
c.nook(2, 2)
def tunnel(c, y, left_entry):
    c.h(4, y-1, 31, 'w'); c.h(4, y+1, 31, 'w'); c.h(5, y, 29, ' ')
    c.p(4, y, ':' if left_entry else 'w'); c.p(34, y, 'w' if left_entry else ':')
    for k in range(4):
        # the pit is two along: one push puts the stone on its edge, and it rolls in
        if left_entry:
            r = 8 + 7*k; pit = r + 2; ds = (r+4, r+5)
        else:
            r = 30 - 7*k; pit = r - 2; ds = (r-4, r-5)
        c.p(r, y, 'r'); c.p(pit, y+1, ' '); c.p(pit, y+2, ' ')
        for d in ds: c.p(d, y, 'd')
tunnel(c, 5, True); tunnel(c, 11, False); tunnel(c, 17, True)
c.p(37, 20, 'X')
c.save('Drumlin', '03-drumlin.txt', about='three long burrows; shove the stones into the holes',
       value='15 20', quota='14 16 18 20 22', time='160 150 140 130 120', seed=seeds(3),
       fill='boulder 30, diamond 6')

c = C()  # fireflies
c.nook(2, 2)
for x, y, w, h in [(6, 5, 7, 3), (18, 4, 6, 4), (29, 6, 7, 3), (6, 13, 6, 3), (19, 14, 8, 3), (31, 13, 5, 4)]:
    c.kettle(x, y, w, h, 'f')
    c.p(x + w//2, y-2, 'r')
    c.p(x-1, y-1, 'd'); c.p(x+w, y+h, 'd')
c.p(37, 20, 'X')
c.save('Kettle', '04-kettle.txt', about='embers in the hollows; drop a stone on them',
       value='15 25', quota='12 14 16 18 20', time='150 140 130 120 110', seed=seeds(4),
       fill='boulder 40, diamond 10')

c = C(20, 12)  # intermission: a quick gather
c.nook(2, 2)
for y in (4, 7, 10):
    c.h(4, y, 13, 'w')
    for i in range(0, 13, 2):
        c.p(4+i, y-1, 'd')
    for i in range(1, 13, 2):
        c.p(4+i, y-1, 'r')
c.p(18, 10, 'X')
c.save('Rime', '05-rime.txt', about='intermission: frost on every ledge, and no time',
       value='20 30', quota='8 9 10 11 12', time='25 22 20 18 16', seed=seeds(5))

# ---- 5-8: butterflies, then pairs, then the magic wall ------------------------

c = C()  # butterflies
c.nook(2, 2)
for x, y, w, h, flies in [(4, 4, 12, 5, [(5, 5), (14, 7)]), (21, 4, 14, 5, [(22, 5), (33, 7)]), (8, 13, 23, 5, [(9, 14), (29, 16)])]:
    c.ring(x, y, w, h)
    for fx, fy in flies: c.p(fx, fy, 'b')
    for i in range(2, w-2):
        c.p(x+i, y, ':')
        c.p(x+i, y-1, 'r' if i % 2 == 0 else ':')
c.p(37, 20, 'X')
c.save('Esker', '06-esker.txt', about='moths on the ridges; crush them for their sparkle',
       value='10 15', quota='14 18 22 26 30', time='150 140 130 120 110', seed=seeds(6),
       fill='boulder 40, diamond 4')

c = C()  # fireflies and butterflies together
c.nook(2, 2)
for k, (x, y) in enumerate([(5, 5), (16, 5), (27, 5), (5, 14), (16, 14), (27, 14)]):
    fly = 'b' if (k + (k // 3)) % 2 == 0 else 'f'
    c.kettle(x, y, 7, 3, fly)
    c.h(x+1, y-2, 5, 'r')
c.p(37, 20, 'X')
c.save('Kame', '07-kame.txt', about='embers and moths side by side; one leaves ash, one leaves gems',
       value='10 20', quota='16 20 24 28 32', time='150 140 130 120 110', seed=seeds(7),
       fill='boulder 50, diamond 6')

c = C()  # magic wall
c.nook(2, 18)
c.h(2, 10, 36, ':')
c.h(3, 11, 34, 'm'); c.p(2, 11, 'w'); c.p(37, 11, 'w')
c.rect(3, 12, 34, 2, ' '); c.v(2, 12, 2, 'w'); c.v(37, 12, 2, 'w'); c.p(2, 13, ':')
c.h(2, 14, 36, 'w')
c.p(37, 20, 'X')
c.save('Rock Flour', '08-rock-flour.txt', about='the ice grinds stone to powder; let the wall grind it to gems',
       value='15 20', quota='12 15 18 21 24', time='150 140 130 120 110', seed=seeds(8),
       fill='boulder 50, diamond 3', magic=40)

c = C(20, 12)  # intermission: one butterfly under a column of stones
c.nook(2, 2)
c.ring(4, 6, 12, 5); c.p(5, 7, 'b'); c.p(14, 9, 'b')
for x in range(6, 14):
    c.p(x, 6, ':')
    c.v(x, 3, 3, 'r' if x % 2 == 0 else ':')
c.p(18, 10, 'X')
c.save('Hoarfrost', '10-hoarfrost.txt', about='intermission: two moths under the stones',
       value='20 30', quota='8 9 10 11 12', time='30 27 24 21 18', seed=seeds(9))

c = C()  # magic wall with creatures
c.nook(2, 2)
c.h(6, 7, 28, ':')
c.h(6, 8, 28, 'm'); c.p(5, 8, 'w'); c.p(34, 8, 'w')
c.rect(6, 9, 28, 3, ' '); c.v(5, 9, 3, 'w'); c.v(34, 9, 3, 'w'); c.p(34, 11, ':')
c.h(5, 12, 30, 'w')
c.p(8, 10, 'b'); c.p(30, 10, 'b')
c.rect(8, 3, 22, 3, 'r')
for x, y in [(4, 15), (16, 16), (28, 15)]:
    c.kettle(x, y, 6, 3, 'f')
c.p(37, 20, 'X')
c.save('Outwash', '09-outwash.txt', about='gems rain into the moths below; mind the embers',
       value='10 15', quota='20 24 28 32 36', time='160 150 140 130 120', seed=seeds(10),
       fill='boulder 30, diamond 6', magic=30)

# ---- 9-12: amoeba ---------------------------------------------------------------

def vault(c, x, y, w, h, seed_at):
    # brick vault of dirt; the moss starts inside; one gap in the roof plugged by a stone
    c.box(x, y, w, h, 'w'); c.rect(x+1, y+1, w-2, h-2, ':')
    gx = x + w//2
    c.p(gx, y, ':'); c.p(gx, y-1, 'r')
    c.p(*seed_at, 'a')

c = C()  # amoeba enclosed
c.nook(2, 2)
vault(c, 5, 6, 12, 7, (7, 10)); vault(c, 23, 9, 12, 8, (32, 14))
c.p(37, 20, 'X')
c.save('Varve', '11-varve.txt', about='moss in the vaults; let it fill them, then take what it leaves',
       value='10 15', quota='20 26 32 38 44', time='170 160 150 140 130', seed=seeds(11),
       fill='boulder 40, diamond 8', amoeba=20)

c = C()  # amoeba too big
c.nook(2, 2)
c.p(20, 17, 'a')
c.h(8, 8, 6, 'w'); c.h(26, 8, 6, 'w'); c.h(15, 13, 10, 'w')
c.p(37, 2, 'X')
c.save('Crevasse', '12-crevasse.txt', about='the moss runs wild; get your gems before it turns to stone',
       value='10 20', quota='12 14 16 18 20', time='140 130 120 110 100', seed=seeds(12),
       fill='boulder 20, diamond 14', amoeba=30)

c = C()  # amoeba and butterflies
c.nook(2, 2)
for x, y in [(5, 4), (27, 4), (5, 15), (27, 15)]:
    c.ring(x, y, 9, 4)
    c.p(x+1, y+1, 'b')
# a brick block between the rings, with channels of dirt carved through it: the moss
# starts in the middle and can only creep along them, into the rings
c.rect(8, 8, 25, 7, 'w')
c.rect(18, 9, 5, 5, ':'); c.p(20, 11, 'a')
for x in (9, 31):
    c.v(x, 7, 9, ':')
c.h(9, 11, 9, ':'); c.h(23, 11, 9, ':')
c.p(37, 20, 'X')
c.save('Nunatak', '13-nunatak.txt', about='moss creeps down the channels to the moths; where they meet, gems',
       value='10 15', quota='16 20 24 28 32', time='160 150 140 130 120', seed=seeds(13),
       fill='boulder 40, diamond 5', amoeba=15)

c = C()  # amoeba and the magic wall
c.nook(2, 2)
c.h(4, 6, 32, ':')
c.h(5, 7, 30, 'm'); c.p(4, 7, 'w'); c.p(35, 7, 'w')
c.rect(5, 8, 30, 3, ' '); c.v(4, 8, 3, 'w'); c.v(35, 8, 3, 'w')
c.h(4, 11, 32, 'w')
c.p(34, 10, 'a'); c.p(4, 10, ':')
c.p(37, 20, 'X')
c.save('Calving', '14-calving.txt', about='grind gems through the wall before the moss fills the basin',
       value='15 20', quota='12 15 18 21 24', time='150 140 130 120 110', seed=seeds(14),
       fill='boulder 55, diamond 4', magic=45, amoeba=30)

c = C(20, 12)  # intermission: pushing
c.nook(2, 2)
c.h(3, 5, 15, 'w'); c.h(3, 7, 15, 'w'); c.h(4, 6, 13, ' '); c.p(3, 6, ':'); c.p(17, 6, 'w')
for r in (5, 11):
    c.p(r, 6, 'r'); c.p(r+2, 7, ' '); c.p(r+2, 8, ' ')
    c.p(r+4, 6, 'd'); c.p(r+5, 6, 'd')
c.p(16, 6, 'd')
c.h(4, 9, 13, 'd')
c.p(18, 10, 'X')
c.save('Frost Heave', '15-frost-heave.txt', about='intermission: two stones, two holes',
       value='20 30', quota='9 10 11 12 13', time='30 27 24 21 18', seed=seeds(15))

# ---- 13-16: everything ----------------------------------------------------------

c = C()  # the bowl
c.nook(2, 2)
for i in range(7):
    c.p(8+i, 6+i, 'w'); c.p(31-i, 6+i, 'w')
c.h(15, 12, 10, 'w')
for y in range(5, 12):
    for x in range(9 + (y-6), 31 - (y-6)):
        if 0 < x < 39 and y >= 5:
            c.p(x, y, 'r' if (x + y) % 3 else 'd')
c.ring(4, 15, 32, 5); c.p(5, 16, 'f'); c.p(34, 18, 'f'); c.p(20, 16, 'b')
for x in range(8, 33, 6): c.p(x, 15, ':')
c.p(37, 2, 'X')
c.save('Cirque', '16-cirque.txt', about='a bowl of stone and gems over a ring of fire',
       value='10 15', quota='20 24 28 32 36', time='160 150 140 130 120', seed=seeds(16),
       fill='boulder 30, diamond 8')

c = C()  # towers
c.nook(2, 19)
for x in (6, 12, 18, 24, 30):
    c.v(x, 8, 12, 'w')
    c.v(x, 3, 5, 'r')
    c.p(x-1, 19, 'd'); c.p(x+1, 19, 'd')
for x in (9, 21):
    c.rect(x-1, 9, 3, 9, ' '); c.p(x, 9, 'b')
for x in (15, 27):
    c.rect(x-1, 9, 3, 9, ' '); c.p(x, 9, 'f')
c.p(37, 2, 'X')
c.save('Serac', '17-serac.txt', about='towers of ice with stones on top, and things flying between',
       value='15 20', quota='14 18 22 25 28', time='160 150 140 130 120', seed=seeds(17),
       fill='boulder 25, diamond 6')

c = C()  # stripes
c.nook(2, 2)
for y in (5, 9):
    c.h(3, y, 34, 'w'); c.h(4, y+1, 32, ' '); c.h(3, y+2, 34, 'w'); c.p(3, y+1, 'w'); c.p(36, y+1, 'w')
    c.p(4, y+1, 'f' if y == 5 else 'b'); c.p(20, y+1, 'b' if y == 5 else 'f')
    for x in range(6, 35, 5): c.p(x, y, ':'); c.p(x, y-1, 'r')
c.h(4, 14, 32, ':'); c.h(5, 15, 30, 'm'); c.p(4, 15, 'w'); c.p(35, 15, 'w')
c.rect(5, 16, 30, 2, ' '); c.v(4, 16, 2, 'w'); c.v(35, 16, 2, 'w'); c.p(35, 17, ':'); c.h(4, 18, 32, 'w')
c.p(37, 20, 'X')
c.save('Striae', '18-striae.txt', about='the ice scored the rock in long lines; something lives in each',
       value='10 20', quota='20 25 30 35 40', time='170 160 150 140 130', seed=seeds(18),
       fill='boulder 45, diamond 5', magic=35)

c = C(20, 12)  # intermission: moss in a box
c.nook(2, 2)
c.box(5, 4, 10, 6, 'w'); c.rect(6, 5, 8, 4, ':'); c.p(9, 4, ':'); c.p(9, 3, 'r'); c.p(7, 6, 'a')
c.p(18, 10, 'X')
c.save('Firn', '20-firn.txt', about='intermission: wait for the moss to close itself in',
       value='15 20', quota='16 18 20 22 24', time='40 36 32 28 24', seed=seeds(19), amoeba=3)

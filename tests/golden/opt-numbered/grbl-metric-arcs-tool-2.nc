N10 (Summary, written by PutiaCNC)
N20 (Zero:                   XY0 = lower-left corner of stock, Z0 = top surface, in G54)
N30 (Start:                  Starts the spindle at 10000 rpm and waits 2 s, then moves to X44.618)
N40 (                        Y20.015.)
N50 (Tools:                  6 mm flat end mill. No tool change.)
N60 (Cutting:                Down to Z -1 mm. 33 mm of cutting, about 7 s in all.)
N70 (End:                    Lifts to Z 5 mm, stops the spindle, goes to X0 Y0.)
N80 (File 2 of 2: 6 mm flat end mill operation 2)
N90 (Before you start: change to 6 mm flat end mill and touch off Z0 again.)
N100 (PutiaCNC - GRBL)
N110 (Stock 60 x 40 x 6 mm; XY0 = lower-left corner of stock, Z0 = top surface)
N120 G21 G90 G17 G94 G54
N130 G0 Z5
N140 (Tool: 6 mm flat end mill)
N150 M3 S10000
N160 G4 P2
N170 (Operation 2: inside, tool 6 mm)
N180 G0 X44.618 Y20.015
N190 G0 Z0.5
N200 G1 Z0 F300
N210 (Pass 1)
N220 G1 Z-1
N230 G1 F1000
N240 G2 X45.382 Y29.985 I0.382 J4.985
N250 G2 X45.116 Y20.001 I-0.382 J-4.985
N260 G1 X44.618 Y20.015
N270 G0 Z5
N280 M5
N290 G0 X0 Y0
N300 M2
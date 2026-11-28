N10 (Summary, written by PutiaCNC)
N20 (Spindle (!):            Not run by the program: it stops -M0- to ask you to start it at the)
N30 (                        speed shown, and again before a tool change and at the end to stop it.)
N40 (                        There is no work coordinate system: jog to the zero of the job and set)
N50 (                        it with G92 X0 Y0 Z0 first.)
N60 (Tools:                  6 mm flat end mill. No tool change.)
N70 (Pauses:                 The program stops 2 times with the spindle off: 'Start the spindle at)
N80 (                        10000 rpm, then resume', 'Stop the spindle, then resume'. Resume it)
N90 (                        when you are ready; the spindle starts again by itself.)
N100 (Cutting:                Down to Z -1 mm. 33 mm of cutting, about 5 s in all.)
N110 (End:                    Lifts to Z 5 mm, goes to X0 Y0.)
N120 (File 2 of 2: 6 mm flat end mill operation 2)
N130 (Before you start: change to 6 mm flat end mill and touch off Z0 again.)
N140 ; PutiaCNC - Marlin (no spindle control)
N150 ; Stock 60 x 40 x 6 mm; XY0 = lower-left corner of stock, Z0 = top surface
N160 G21 G90
N170 ; Marlin: jog the tool to the zero of the job and set it there with G92 X0 Y0 Z0 before you run this file
N180 ; Spindle: by hand. The program stops to ask you to start and stop it
N190 G0 Z5
N200 ; Tool: 6 mm flat end mill
N210 M0 Start the spindle at 10000 rpm, then resume
N220 ; Operation 2: inside, tool 6 mm
N230 G0 X44.618 Y20.015
N240 G0 Z0.5
N250 G1 Z0 F300
N260 ; Pass 1
N270 G1 Z-1
N280 G1 F1000
N290 G2 X45.382 Y29.985 I0.382 J4.985
N300 G2 X45.116 Y20.001 I-0.382 J-4.985
N310 G1 X44.618 Y20.015
N320 G0 Z5
N330 M0 Stop the spindle, then resume
N340 G0 X0 Y0
N350 ; End of program
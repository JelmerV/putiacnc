N10 (Summary, written by PutiaCNC)
N20 (Spindle (!):            Not run by the program: it stops -M0- to ask you to start it at the)
N30 (                        speed shown, and again before a tool change and at the end to stop it.)
N40 (                        There is no work coordinate system: jog to the zero of the job and set)
N50 (                        it with G92 X0 Y0 Z0 first.)
N60 (Tools:                  3 mm flat end mill. No tool change.)
N70 (Pauses:                 The program stops 2 times with the spindle off: 'Start the spindle at)
N80 (                        12000 rpm, then resume', 'Stop the spindle, then resume'. Resume it)
N90 (                        when you are ready; the spindle starts again by itself.)
N100 (Cutting:                Down to Z -1 mm. 61 mm of cutting, about 7 s in all.)
N110 (End:                    Lifts to Z 5 mm, goes to X0 Y0.)
N120 (File 1 of 2: 3 mm flat end mill operation 1)
N130 ; PutiaCNC - Marlin (no spindle control)
N140 ; Stock 60 x 40 x 6 mm; XY0 = lower-left corner of stock, Z0 = top surface
N150 G21 G90
N160 ; Marlin: jog the tool to the zero of the job and set it there with G92 X0 Y0 Z0 before you run this file
N170 ; Spindle: by hand. The program stops to ask you to start and stop it
N180 G0 Z5
N190 ; Tool: 3 mm flat end mill
N200 M0 Start the spindle at 12000 rpm, then resume
N210 ; Operation 1: outside, tool 3 mm
N220 G0 X20.344 Y3.54
N230 G0 Z0.5
N240 G1 Z0 F250
N250 ; Pass 1
N260 G1 Z-1
N270 G1 F800
N280 G1 X20.669 Y3.657
N290 G1 X20.959 Y3.846
N300 G1 X21.197 Y4.096
N310 G1 X21.372 Y4.395
N320 G1 X21.475 Y4.725
N330 G1 X21.5 Y5
N340 G1 Y15
N350 G1 X21.46 Y15.344
N360 G1 X21.343 Y15.669
N370 G1 X21.154 Y15.959
N380 G1 X20.904 Y16.197
N390 G1 X20.605 Y16.372
N400 G1 X20.275 Y16.475
N410 G1 X20 Y16.5
N420 G1 X5
N430 G1 X4.656 Y16.46
N440 G1 X4.331 Y16.343
N450 G1 X4.041 Y16.154
N460 G1 X3.803 Y15.904
N470 G1 X3.628 Y15.605
N480 G1 X3.525 Y15.275
N490 G1 X3.5 Y15
N500 G1 Y5
N510 G1 X3.54 Y4.656
N520 G1 X3.657 Y4.331
N530 G1 X3.846 Y4.041
N540 G1 X4.096 Y3.803
N550 G1 X4.395 Y3.628
N560 G1 X4.725 Y3.525
N570 G1 X5 Y3.5
N580 G1 X20
N590 G1 X20.344 Y3.54
N600 G0 Z5
N610 M0 Stop the spindle, then resume
N620 G0 X0 Y0
N630 ; End of program
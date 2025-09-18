<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform" xmlns:fo="http://www.w3.org/1999/XSL/Format">
    <!-- Page Layout Definition -->
    <fo:simple-page-master page-width="8.5in" page-height="11in" margin="40pt"/>

    <!-- Reusable Style Definitions -->
    <xsl:attribute-set name="th">
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="text-align">left</xsl:attribute>
        <xsl:attribute name="color">#ffffff</xsl:attribute>
        <xsl:attribute name="background-color">#465569</xsl:attribute>
        <xsl:attribute name="padding">8pt 5pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td">
        <xsl:attribute name="padding">8pt 5pt</xsl:attribute>
        <xsl:attribute name="border-bottom">0.5pt solid #dddddd</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td-right">
        <xsl:attribute name="padding">8pt 5pt</xsl:attribute>
        <xsl:attribute name="text-align">right</xsl:attribute>
        <xsl:attribute name="border-bottom">0.5pt solid #dddddd</xsl:attribute>
    </xsl:attribute-set>

    <!-- Main Template -->
    <xsl:template match="/">
        <page-sequence select="customers">
            <!-- Header Section -->
            <flex-container margin-bottom="20pt">
                <container width="50%">
                    <text font-size="28pt" font-weight="bold" color="#003366">INVOICE</text>
                </container>
                <container width="50%" text-align="right" font-size="10pt" color="#666666">
                    <text>Invoice #: <xsl:value-of select="invoiceNumber"/></text>
                    <text>Date: October 27, 2023</text>
                </container>
            </flex-container>

            <!-- Bill To Section -->
            <container margin-bottom="30pt">
                <text font-size="10pt" font-weight="bold" color="#666666" margin-bottom="5pt">BILL TO</text>
                <text font-size="11pt"><xsl:value-of select="name"/></text>
                <text font-size="11pt"><xsl:value-of select="address"/></text>
            </container>

            <!-- Items Table -->
            <table width="100%">
                <columns>
                    <column width="50%"/>
                    <column width="25%"/>
                    <column width="25%"/>
                </columns>
                <header>
                    <row>
                        <cell style="th">Product / Service</cell>
                        <cell style="th" text-align="right">Quantity</cell>
                        <cell style="th" text-align="right">Price</cell>
                    </row>
                </header>
                <tbody>
                    <xsl:for-each select="items">
                        <row font-size="10pt">
                            <cell style="td"><xsl:value-of select="product"/></cell>
                            <cell style="td-right"><xsl:value-of select="quantity"/></cell>
                            <cell style="td-right"><xsl:value-of select="price"/></cell>
                        </row>
                    </xsl:for-each>
                </tbody>
            </table>
        </page-sequence>
    </xsl:template>
</xsl:stylesheet>